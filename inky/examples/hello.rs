//! Simple hello world example demonstrating inky's basic features.
//!
//! Displays a styled box with "Hello, inky!" text.
//! Press 'q' or Ctrl+C to exit.

use inky::prelude::*;

fn main() -> Result<()> {
    App::new()
        .alt_screen(true)
        .render(|ctx| {
            // Center the box by wrapping in a container
            BoxNode::new()
                .width(ctx.width())
                .height(ctx.height())
                .justify_content(JustifyContent::Center)
                .align_items(AlignItems::Center)
                .child(
                    BoxNode::new()
                        .width(40)
                        .height(5)
                        .border(BorderStyle::Rounded)
                        .justify_content(JustifyContent::Center)
                        .align_items(AlignItems::Center)
                        .child(
                            TextNode::new("Hello, inky!")
                                .color(Color::BrightCyan)
                                .bold(),
                        ),
                )
                .into()
        })
        .on_key(|_state, key| {
            // Quit on 'q' or Escape
            matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
        })
        .run()?;

    Ok(())
}
