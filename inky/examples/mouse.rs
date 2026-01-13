//! Mouse interaction example demonstrating click, hover, and drag-and-drop.
//!
//! This example shows how to use inky's mouse interaction features:
//! - Basic click handling with `Clickable` component
//! - Hover state tracking with visual feedback
//! - Drag-and-drop between zones with `Draggable` and `DropZone`
//! - Raw mouse event handling via `on_mouse`
//!
//! Controls:
//! - Mouse click: Click buttons to see click count
//! - Mouse drag: Drag items between drop zones
//! - Right click: Show context menu coordinates
//! - Scroll: Scroll up/down to change scroll counter
//! - q / Escape: Quit

use inky::components::{Clickable, Draggable, DropZone};
use inky::prelude::*;
use std::sync::Arc;

/// Application state tracking mouse interactions.
struct MouseState {
    /// Number of left clicks on the primary button.
    click_count: Signal<u32>,
    /// Number of right clicks.
    right_click_count: Signal<u32>,
    /// Current mouse position.
    mouse_pos: Signal<(u16, u16)>,
    /// Last click position.
    last_click: Signal<Option<(u16, u16)>>,
    /// Scroll delta (positive = up, negative = down).
    scroll_delta: Signal<i32>,
    /// Items in the left zone.
    left_items: Signal<Vec<String>>,
    /// Items in the right zone.
    right_items: Signal<Vec<String>>,
    /// Status message.
    status: Signal<String>,
}

impl MouseState {
    fn new() -> Self {
        Self {
            click_count: use_signal(0),
            right_click_count: use_signal(0),
            mouse_pos: use_signal((0, 0)),
            last_click: use_signal(None),
            scroll_delta: use_signal(0),
            left_items: use_signal(vec![
                "Item A".to_string(),
                "Item B".to_string(),
                "Item C".to_string(),
            ]),
            right_items: use_signal(Vec::new()),
            status: use_signal("Move mouse or click to interact".to_string()),
        }
    }
}

fn main() -> Result<()> {
    let state = MouseState::new();

    App::new()
        .state(state)
        .render(|ctx| {
            let click_count = ctx.state.click_count.get();
            let right_count = ctx.state.right_click_count.get();
            let (mx, my) = ctx.state.mouse_pos.get();
            let last_click = ctx.state.last_click.get();
            let scroll = ctx.state.scroll_delta.get();
            let left_items = ctx.state.left_items.get();
            let right_items = ctx.state.right_items.get();
            let status = ctx.state.status.get();

            // Clone signals for use in closures
            let click_signal = ctx.state.click_count.clone();
            let status_signal = ctx.state.status.clone();

            BoxNode::new()
                .flex_direction(FlexDirection::Column)
                .width(Dimension::Percent(1.0))
                .height(Dimension::Percent(1.0))
                .padding(1)
                .child(
                    // Title
                    TextNode::new("Mouse Interaction Demo")
                        .bold()
                        .color(Color::Cyan),
                )
                .child(TextNode::new(""))
                // Mouse info row
                .child(
                    BoxNode::new()
                        .flex_direction(FlexDirection::Row)
                        .gap(2.0)
                        .child(
                            TextNode::new(format!("Position: ({}, {})", mx, my))
                                .color(Color::Yellow),
                        )
                        .child(TextNode::new(format!("Left clicks: {}", click_count)))
                        .child(TextNode::new(format!("Right clicks: {}", right_count)))
                        .child(TextNode::new(format!(
                            "Scroll: {}{}",
                            if scroll >= 0 { "+" } else { "" },
                            scroll
                        ))),
                )
                .child(
                    TextNode::new(if let Some((x, y)) = last_click {
                        format!("Last click at: ({}, {})", x, y)
                    } else {
                        "Last click at: (none)".to_string()
                    })
                    .color(Color::BrightBlack),
                )
                .child(TextNode::new(""))
                // Clickable buttons row
                .child(
                    BoxNode::new()
                        .flex_direction(FlexDirection::Row)
                        .gap(2.0)
                        .child(TextNode::new("Clickable buttons: "))
                        .child(
                            Clickable::new(
                                BoxNode::new()
                                    .padding_xy(2.0, 0.0)
                                    .border(BorderStyle::Single)
                                    .child(TextNode::new("Click Me!").color(Color::Green)),
                            )
                            .on_click({
                                let click_signal = click_signal.clone();
                                let status_signal = status_signal.clone();
                                move |event| {
                                    click_signal.update(|c| *c += 1);
                                    status_signal.set(format!(
                                        "Button clicked at ({}, {})",
                                        event.local_x, event.local_y
                                    ));
                                }
                            })
                            .hover_background(Color::Blue),
                        )
                        .child(
                            Clickable::new(
                                BoxNode::new()
                                    .padding_xy(2.0, 0.0)
                                    .border(BorderStyle::Single)
                                    .child(TextNode::new("Hover Me!").color(Color::Magenta)),
                            )
                            .on_hover({
                                let status_signal = status_signal.clone();
                                move || {
                                    status_signal.set("Hovering over button!".to_string());
                                }
                            })
                            .on_unhover({
                                let status_signal = status_signal.clone();
                                move || {
                                    status_signal.set("Mouse left the button".to_string());
                                }
                            })
                            .hover_background(Color::Rgb(60, 60, 80)),
                        )
                        .child(
                            Clickable::new(
                                BoxNode::new()
                                    .padding_xy(2.0, 0.0)
                                    .border(BorderStyle::Single)
                                    .background(Color::BrightBlack)
                                    .child(TextNode::new("Disabled").color(Color::White)),
                            )
                            .disabled(true),
                        ),
                )
                .child(TextNode::new(""))
                // Drag and drop section
                .child(TextNode::new("Drag and Drop:").bold())
                .child(
                    BoxNode::new()
                        .flex_direction(FlexDirection::Row)
                        .gap(4.0)
                        .margin_xy(0.0, 1.0)
                        // Left drop zone
                        .child(
                            DropZone::new(
                                BoxNode::new()
                                    .flex_direction(FlexDirection::Column)
                                    .width(Dimension::Length(20.0))
                                    .min_height(Dimension::Length(6.0))
                                    .border(BorderStyle::Double)
                                    .padding(1)
                                    .child(TextNode::new("Left Zone").color(Color::Cyan))
                                    .children(left_items.iter().map(|item| -> Node {
                                        Draggable::new(
                                            BoxNode::new()
                                                .border(BorderStyle::Single)
                                                .padding_xy(1.0, 0.0)
                                                .margin_xy(0.0, 1.0)
                                                .child(TextNode::new(item.clone())),
                                        )
                                        .drag_data(Arc::new(item.clone()))
                                        .into()
                                    })),
                            )
                            .on_drag_enter({
                                let status_signal = status_signal.clone();
                                move || {
                                    status_signal.set("Dragging over left zone".to_string());
                                }
                            })
                            .on_drop({
                                let status_signal = status_signal.clone();
                                move |event| {
                                    status_signal.set(format!(
                                        "Dropped at ({}, {}) in left zone",
                                        event.drop_x, event.drop_y
                                    ));
                                }
                            })
                            .drag_over_background(Color::Rgb(30, 50, 30)),
                        )
                        // Right drop zone
                        .child(
                            DropZone::new(
                                BoxNode::new()
                                    .flex_direction(FlexDirection::Column)
                                    .width(Dimension::Length(20.0))
                                    .min_height(Dimension::Length(6.0))
                                    .border(BorderStyle::Double)
                                    .padding(1)
                                    .child(TextNode::new("Right Zone").color(Color::Yellow))
                                    .children(right_items.iter().map(|item| -> Node {
                                        BoxNode::new()
                                            .border(BorderStyle::Single)
                                            .padding_xy(1.0, 0.0)
                                            .margin_xy(0.0, 1.0)
                                            .child(TextNode::new(item.clone()))
                                            .into()
                                    })),
                            )
                            .on_drag_enter({
                                let status_signal = status_signal.clone();
                                move || {
                                    status_signal.set("Dragging over right zone".to_string());
                                }
                            })
                            .on_drop({
                                let status_signal = status_signal.clone();
                                move |event| {
                                    status_signal.set(format!(
                                        "Dropped at ({}, {}) in right zone",
                                        event.drop_x, event.drop_y
                                    ));
                                }
                            })
                            .drag_over_background(Color::Rgb(50, 50, 30)),
                        ),
                )
                .child(TextNode::new(""))
                // Status line
                .child(
                    BoxNode::new()
                        .border(BorderStyle::Single)
                        .padding_xy(1.0, 0.0)
                        .child(TextNode::new(format!("Status: {}", status)).color(Color::White)),
                )
                .child(TextNode::new(""))
                // Help text
                .child(
                    TextNode::new("Click buttons | Drag items | Scroll | Right-click | q: quit")
                        .color(Color::BrightBlack),
                )
                .into()
        })
        .on_mouse(|state, mouse| {
            // Track mouse position
            state.mouse_pos.set((mouse.x, mouse.y));

            match mouse.kind {
                MouseEventKind::Down => {
                    if let Some(button) = mouse.button {
                        match button {
                            MouseButton::Left => {
                                state.last_click.set(Some((mouse.x, mouse.y)));
                            }
                            MouseButton::Right => {
                                state.right_click_count.update(|c| *c += 1);
                                state
                                    .status
                                    .set(format!("Right-click at ({}, {})", mouse.x, mouse.y));
                            }
                            MouseButton::Middle => {
                                // Reset scroll on middle click
                                state.scroll_delta.set(0);
                                state.status.set("Scroll reset".to_string());
                            }
                        }
                    }
                }
                MouseEventKind::ScrollUp => {
                    state.scroll_delta.update(|d| *d += 1);
                }
                MouseEventKind::ScrollDown => {
                    state.scroll_delta.update(|d| *d -= 1);
                }
                _ => {}
            }

            false // Don't quit
        })
        .on_key(|_state, key| matches!(key.code, KeyCode::Char('q') | KeyCode::Esc))
        .run()?;

    Ok(())
}
