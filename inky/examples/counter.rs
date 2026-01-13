//! Reactive counter example demonstrating Signal<T> hooks.
//!
//! This example shows how to use signals for reactive state management.
//! The counter value is stored in a Signal, which automatically triggers
//! re-renders when the value changes.
//!
//! Controls:
//! - Up Arrow / k: Increment counter
//! - Down Arrow / j: Decrement counter
//! - r: Reset counter to 0
//! - q / Escape: Quit

use inky::prelude::*;

/// Application state using reactive signals.
struct CounterState {
    /// The counter value stored in a reactive signal.
    count: Signal<i32>,
}

impl CounterState {
    fn new() -> Self {
        Self {
            count: use_signal(0),
        }
    }

    fn increment(&self) {
        self.count.update(|c| *c += 1);
    }

    fn decrement(&self) {
        self.count.update(|c| *c -= 1);
    }

    fn reset(&self) {
        self.count.set(0);
    }
}

fn main() -> Result<()> {
    let state = CounterState::new();

    App::new()
        .state(state)
        .render(|ctx| {
            let count = ctx.state.count.get();

            // Create the UI tree
            BoxNode::new()
                .flex_direction(FlexDirection::Column)
                .align_items(AlignItems::Center)
                .justify_content(JustifyContent::Center)
                .width(Dimension::Percent(1.0))
                .height(Dimension::Percent(1.0))
                .child(
                    BoxNode::new()
                        .flex_direction(FlexDirection::Column)
                        .align_items(AlignItems::Center)
                        .padding(2)
                        .border(BorderStyle::Rounded)
                        .child(TextNode::new("Reactive Counter").bold().color(Color::Cyan))
                        .child(TextNode::new(""))
                        .child(TextNode::new(format!("Count: {}", count)).bold().color(
                            if count > 0 {
                                Color::Green
                            } else if count < 0 {
                                Color::Red
                            } else {
                                Color::White
                            },
                        ))
                        .child(TextNode::new(""))
                        .child(
                            TextNode::new("Up/k: +1  Down/j: -1  r: reset  q: quit")
                                .color(Color::BrightBlack),
                        ),
                )
                .into()
        })
        .on_key(|state, key| {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    state.increment();
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    state.decrement();
                }
                KeyCode::Char('r') => {
                    state.reset();
                }
                KeyCode::Char('q') | KeyCode::Esc => {
                    return true; // Quit
                }
                _ => {}
            }
            false // Continue running
        })
        .run()?;

    Ok(())
}
