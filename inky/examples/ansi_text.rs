//! ANSI text passthrough example demonstrating ANSI escape sequence rendering.
#![allow(clippy::format_push_string, clippy::many_single_char_names)] // Example code prioritizes readability
//!
//! This example shows how inky handles ANSI escape sequences in text:
//! - Colors (16-color, 256-color, and true color)
//! - Text attributes (bold, italic, underline, dim, strikethrough)
//! - Reset sequences
//!
//! This is useful for:
//! - Displaying output from other CLI tools (ls --color, git diff, etc.)
//! - Rendering syntax-highlighted code
//! - Porting existing ANSI-colored output to inky
//!
//! Run with:
//! ```bash
//! cargo run --example ansi_text
//! ```
//!
//! Controls:
//! - Tab: Switch between example panels
//! - q / Escape: Quit

use inky::prelude::*;

/// Different panels showing ANSI features.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Panel {
    #[default]
    Colors16,
    Colors256,
    TrueColor,
    Attributes,
    Combined,
}

impl Panel {
    fn next(self) -> Self {
        match self {
            Panel::Colors16 => Panel::Colors256,
            Panel::Colors256 => Panel::TrueColor,
            Panel::TrueColor => Panel::Attributes,
            Panel::Attributes => Panel::Combined,
            Panel::Combined => Panel::Colors16,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Panel::Colors16 => "16 Colors",
            Panel::Colors256 => "256 Colors",
            Panel::TrueColor => "True Color",
            Panel::Attributes => "Attributes",
            Panel::Combined => "Combined",
        }
    }
}

/// Application state.
struct AppState {
    panel: Signal<Panel>,
}

impl AppState {
    fn new() -> Self {
        Self {
            panel: use_signal(Panel::Colors16),
        }
    }
}

/// Generate ANSI 16-color demo text.
fn ansi_16_colors() -> String {
    let mut s = String::new();

    // Standard colors (0-7)
    s.push_str("Standard colors:\n");
    for i in 30..=37 {
        s.push_str(&format!("\x1b[{}m color {} \x1b[0m", i, i - 30));
    }
    s.push_str("\n\n");

    // Bright colors (90-97)
    s.push_str("Bright colors:\n");
    for i in 90..=97 {
        s.push_str(&format!("\x1b[{}m bright {} \x1b[0m", i, i - 90));
    }
    s.push_str("\n\n");

    // Background colors
    s.push_str("Background colors:\n");
    for i in 40..=47 {
        s.push_str(&format!("\x1b[{}m  bg{}  \x1b[0m", i, i - 40));
    }
    s.push('\n');
    for i in 100..=107 {
        s.push_str(&format!("\x1b[{}m bright \x1b[0m", i));
    }
    s.push('\n');

    s
}

/// Generate ANSI 256-color demo text.
fn ansi_256_colors() -> String {
    let mut s = String::new();

    // Color cube (16-231)
    s.push_str("256-color palette (sample):\n\n");

    // Show a subset of the 6x6x6 color cube
    for row in 0..6 {
        for col in 0..12 {
            let color = 16 + row * 36 + col * 3;
            if color <= 231 {
                s.push_str(&format!("\x1b[38;5;{}m███\x1b[0m", color));
            }
        }
        s.push('\n');
    }
    s.push('\n');

    // Grayscale (232-255)
    s.push_str("Grayscale:\n");
    for i in 232..=255 {
        s.push_str(&format!("\x1b[38;5;{}m█\x1b[0m", i));
    }
    s.push('\n');

    s
}

/// Generate true color demo text.
fn ansi_true_color() -> String {
    let mut s = String::new();

    s.push_str("True color (24-bit RGB):\n\n");

    // Red gradient
    s.push_str("Red:    ");
    for i in (0..=255).step_by(16) {
        s.push_str(&format!("\x1b[38;2;{};0;0m█\x1b[0m", i));
    }
    s.push('\n');

    // Green gradient
    s.push_str("Green:  ");
    for i in (0..=255).step_by(16) {
        s.push_str(&format!("\x1b[38;2;0;{};0m█\x1b[0m", i));
    }
    s.push('\n');

    // Blue gradient
    s.push_str("Blue:   ");
    for i in (0..=255).step_by(16) {
        s.push_str(&format!("\x1b[38;2;0;0;{}m█\x1b[0m", i));
    }
    s.push('\n');

    // Rainbow
    s.push_str("\nRainbow:\n");
    for i in (0..=360).step_by(4) {
        let (r, g, b) = hsv_to_rgb(i as f32, 1.0, 1.0);
        s.push_str(&format!("\x1b[38;2;{};{};{}m█\x1b[0m", r, g, b));
    }
    s.push('\n');

    // Pastel gradient
    s.push_str("\nPastels:\n");
    for i in (0..=360).step_by(4) {
        let (r, g, b) = hsv_to_rgb(i as f32, 0.4, 1.0);
        s.push_str(&format!("\x1b[38;2;{};{};{}m█\x1b[0m", r, g, b));
    }
    s.push('\n');

    s
}

/// Generate text attribute demo.
fn ansi_attributes() -> String {
    let mut s = String::new();

    s.push_str("Text attributes:\n\n");

    s.push_str("\x1b[1mBold text\x1b[0m\n");
    s.push_str("\x1b[2mDim text\x1b[0m\n");
    s.push_str("\x1b[3mItalic text\x1b[0m\n");
    s.push_str("\x1b[4mUnderlined text\x1b[0m\n");
    s.push_str("\x1b[9mStrikethrough text\x1b[0m\n");
    s.push_str("\x1b[7mInverse/reverse text\x1b[0m\n");
    s.push('\n');

    s.push_str("Combinations:\n\n");
    s.push_str("\x1b[1;3mBold + Italic\x1b[0m\n");
    s.push_str("\x1b[1;4mBold + Underline\x1b[0m\n");
    s.push_str("\x1b[3;4mItalic + Underline\x1b[0m\n");
    s.push_str("\x1b[1;3;4mBold + Italic + Underline\x1b[0m\n");

    s
}

/// Generate combined demo with realistic use case.
fn ansi_combined() -> String {
    let mut s = String::new();

    // Simulated git diff output
    s.push_str("\x1b[1mSimulated git diff:\x1b[0m\n\n");
    s.push_str("\x1b[1;34mdiff --git a/src/main.rs b/src/main.rs\x1b[0m\n");
    s.push_str("\x1b[36m@@ -10,6 +10,8 @@\x1b[0m\n");
    s.push_str(" fn main() {\n");
    s.push_str("\x1b[31m-    println!(\"Hello\");\x1b[0m\n");
    s.push_str("\x1b[32m+    println!(\"Hello, World!\");\x1b[0m\n");
    s.push_str("\x1b[32m+    println!(\"Welcome to inky!\");\x1b[0m\n");
    s.push_str(" }\n");
    s.push('\n');

    // Simulated ls output
    s.push_str("\x1b[1mSimulated ls --color:\x1b[0m\n\n");
    s.push_str("\x1b[34mbin/\x1b[0m  ");
    s.push_str("\x1b[34msrc/\x1b[0m  ");
    s.push_str("\x1b[32mCargo.toml\x1b[0m  ");
    s.push_str("\x1b[33mREADME.md\x1b[0m  ");
    s.push_str("\x1b[31;1mtarget/\x1b[0m\n");
    s.push('\n');

    // Syntax highlighted code
    s.push_str("\x1b[1mSyntax highlighting:\x1b[0m\n\n");
    s.push_str("\x1b[34mfn\x1b[0m \x1b[33mmain\x1b[0m() {\n");
    s.push_str("    \x1b[34mlet\x1b[0m message = \x1b[32m\"Hello\"\x1b[0m;\n");
    s.push_str("    \x1b[35mprintln!\x1b[0m(\x1b[32m\"{}\"\x1b[0m, message);\n");
    s.push_str("}\n");

    s
}

/// Convert HSV to RGB (for rainbow gradient).
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let h = h % 360.0;
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

fn main() -> Result<()> {
    let state = AppState::new();

    App::new()
        .state(state)
        .alt_screen(true)
        .on_key(|state, event| {
            match event.code {
                KeyCode::Char('q') | KeyCode::Esc => return true,
                KeyCode::Tab => state.panel.update(|p| *p = p.next()),
                _ => {}
            }
            false
        })
        .render(|ctx| {
            let panel = ctx.state.panel.get();

            // Generate ANSI text for current panel
            let ansi_content = match panel {
                Panel::Colors16 => ansi_16_colors(),
                Panel::Colors256 => ansi_256_colors(),
                Panel::TrueColor => ansi_true_color(),
                Panel::Attributes => ansi_attributes(),
                Panel::Combined => ansi_combined(),
            };

            BoxNode::new()
                .width(ctx.width())
                .height(ctx.height())
                .flex_direction(FlexDirection::Column)
                .padding(1)
                .child(
                    // Header
                    BoxNode::new()
                        .width(Dimension::Percent(100.0))
                        .border(BorderStyle::Rounded)
                        .padding_xy(1.0, 0.0)
                        .margin_xy(0.0, 1.0)
                        .flex_direction(FlexDirection::Row)
                        .child(
                            TextNode::new("ANSI Text Passthrough Demo")
                                .color(Color::BrightCyan)
                                .bold(),
                        )
                        .child(Spacer::new())
                        .child(
                            TextNode::new(format!("Panel: {}", panel.name())).color(Color::Yellow),
                        ),
                )
                .child(
                    // Description
                    BoxNode::new()
                        .width(Dimension::Percent(100.0))
                        .margin_xy(0.0, 1.0)
                        .child(
                            TextNode::new(
                                "inky parses ANSI escape sequences and renders them natively.",
                            )
                            .color(Color::BrightBlack),
                        ),
                )
                .child(
                    // ANSI content area - uses TextNode::from_ansi()
                    BoxNode::new()
                        .flex_grow(1.0)
                        .width(Dimension::Percent(100.0))
                        .border(BorderStyle::Single)
                        .padding(1)
                        .child(
                            // This is the key API: TextNode::from_ansi() parses ANSI codes
                            TextNode::from_ansi(&ansi_content),
                        ),
                )
                .child(
                    // Controls
                    BoxNode::new()
                        .width(Dimension::Percent(100.0))
                        .margin_xy(0.0, 1.0)
                        .child(
                            TextNode::new("[Tab] Next panel  [q] Quit").color(Color::BrightBlack),
                        ),
                )
                .into()
        })
        .run()?;

    Ok(())
}
