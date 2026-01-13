//! Adaptive rendering example demonstrating tier degradation.
//!
//! This example shows how inky components adapt their rendering based on
//! terminal capabilities. Use keyboard shortcuts to simulate different tiers:
//!
//! - `0` - Tier 0: Dumb terminal (text only, no colors)
//! - `1` - Tier 1: ANSI terminal (256 colors, basic Unicode)
//! - `2` - Tier 2: Modern terminal (true color, synchronized output)
//! - `3` - Tier 3: GPU terminal (dashterm2/dterm acceleration)
//! - `a` - Auto-detect terminal capabilities
//! - `Tab` - Switch between component panels (Visualization, Code, Diff)
//! - `q` or `Esc` - Quit
//!
//! Run with: `cargo run --example adaptive`

use inky::components::{
    ChatMessage, ChatView, DiffLine, DiffView, Markdown, MessageRole, StatusBar, StatusState,
};
use inky::prelude::*;
use std::time::Duration;

/// Panel selection for different component demonstrations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Panel {
    #[default]
    Visualization,
    Code,
    Diff,
    Chat,
}

impl Panel {
    fn next(self) -> Self {
        match self {
            Panel::Visualization => Panel::Code,
            Panel::Code => Panel::Diff,
            Panel::Diff => Panel::Chat,
            Panel::Chat => Panel::Visualization,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Panel::Visualization => "Visualization",
            Panel::Code => "Code/Markdown",
            Panel::Diff => "Diff",
            Panel::Chat => "Chat/Status",
        }
    }
}

/// Application state tracking the selected tier, animation, and panel.
struct AdaptiveState {
    /// Currently selected rendering tier.
    tier: RenderTier,
    /// Whether we're in auto-detect mode.
    auto_detect: bool,
    /// Animation timer for live updates.
    timer: IntervalHandle,
    /// Currently selected panel.
    panel: Panel,
}

impl AdaptiveState {
    fn new() -> Self {
        Self {
            tier: RenderTier::Tier2Retained,
            auto_detect: true,
            timer: use_interval(Duration::from_millis(100)),
            panel: Panel::default(),
        }
    }
}

/// Generate sample heatmap data with animation.
fn generate_heatmap_data(rows: usize, cols: usize, phase: f32) -> Vec<Vec<f32>> {
    let mut data = Vec::with_capacity(rows);
    let row_scale = std::f32::consts::TAU / rows as f32;
    let col_scale = std::f32::consts::TAU / cols as f32;

    for row in 0..rows {
        let mut row_data = Vec::with_capacity(cols);
        let y = row as f32 * row_scale;
        for col in 0..cols {
            let x = col as f32 * col_scale;
            // Create a wave pattern that moves over time
            let value = (x + phase).sin() * 0.5 + 0.5;
            let ripple = (y - phase * 0.7).cos() * 0.5 + 0.5;
            row_data.push(value * ripple);
        }
        data.push(row_data);
    }
    data
}

/// Generate sample sparkline data with animation.
fn generate_sparkline_data(len: usize, phase: f32, freq: f32, base: f32) -> Vec<f32> {
    (0..len)
        .map(|i| {
            let t = phase + i as f32 * 0.15;
            let wave = (t * freq).sin() * 0.5 + 0.5;
            base + wave * 50.0
        })
        .collect()
}

/// Sample markdown content for demonstration.
const SAMPLE_MARKDOWN: &str = r#"# Adaptive Rendering

This **Markdown** component adapts to terminal capabilities.

## Features

- **Bold** and *italic* text
- Inline `code` highlighting
- Lists and headings

### Code Example

```rust
fn main() {
    println!("Hello!");
}
```

> Blockquotes render differently at each tier.
"#;

fn main() -> Result<()> {
    let state = AdaptiveState::new();

    App::new()
        .state(state)
        .alt_screen(true)
        .render(|ctx| {
            let tick = ctx.state.timer.get() as f32;
            let phase = tick * 0.1;

            // Get the effective tier (auto-detected or manually selected)
            let tier = if ctx.state.auto_detect {
                Capabilities::detect().tier()
            } else {
                ctx.state.tier
            };

            // Build the UI
            BoxNode::new()
                .width(ctx.width())
                .height(ctx.height())
                .flex_direction(FlexDirection::Column)
                .padding(1)
                .gap(1.0)
                // Header
                .child(
                    BoxNode::new()
                        .flex_direction(FlexDirection::Row)
                        .justify_content(JustifyContent::SpaceBetween)
                        .child(TextNode::new("inky Adaptive Rendering Demo").bold().color(
                            if tier.supports_true_color() {
                                Color::Rgb(100, 200, 255)
                            } else {
                                Color::BrightCyan
                            },
                        ))
                        .child(render_tier_badge(tier, ctx.state.auto_detect)),
                )
                // Panel tabs
                .child(render_panel_tabs(ctx.state.panel, tier))
                // Main content based on selected panel
                .child(render_panel_content(ctx.state.panel, tier, phase))
                // Tier descriptions
                .child(render_tier_info(tier))
                // Footer with controls
                .child(
                    BoxNode::new()
                        .flex_direction(FlexDirection::Row)
                        .gap(2.0)
                        .child(TextNode::new("Keys:").bold())
                        .child(TextNode::new("0-3").color(Color::BrightYellow))
                        .child(TextNode::new("tier"))
                        .child(TextNode::new("Tab").color(Color::BrightYellow))
                        .child(TextNode::new("panel"))
                        .child(TextNode::new("a").color(Color::BrightYellow))
                        .child(TextNode::new("auto"))
                        .child(TextNode::new("q").color(Color::BrightYellow))
                        .child(TextNode::new("quit")),
                )
                .into()
        })
        .on_key(|state, key| match key.code {
            KeyCode::Char('0') => {
                state.tier = RenderTier::Tier0Fallback;
                state.auto_detect = false;
                false
            }
            KeyCode::Char('1') => {
                state.tier = RenderTier::Tier1Ansi;
                state.auto_detect = false;
                false
            }
            KeyCode::Char('2') => {
                state.tier = RenderTier::Tier2Retained;
                state.auto_detect = false;
                false
            }
            KeyCode::Char('3') => {
                state.tier = RenderTier::Tier3Gpu;
                state.auto_detect = false;
                false
            }
            KeyCode::Char('a' | 'A') => {
                state.auto_detect = true;
                false
            }
            KeyCode::Tab => {
                state.panel = state.panel.next();
                false
            }
            KeyCode::Char('q') | KeyCode::Esc => true,
            _ => false,
        })
        .run()?;

    Ok(())
}

/// Render panel tabs.
fn render_panel_tabs(current: Panel, tier: RenderTier) -> Node {
    let panels = [Panel::Visualization, Panel::Code, Panel::Diff, Panel::Chat];

    let mut tabs = BoxNode::new().flex_direction(FlexDirection::Row).gap(2.0);

    for panel in panels {
        let is_selected = panel == current;
        let mut tab = TextNode::new(format!("[{}]", panel.name()));

        if is_selected {
            tab = tab.bold();
            if tier.supports_true_color() {
                tab = tab.color(Color::Rgb(100, 255, 100));
            } else {
                tab = tab.color(Color::BrightGreen);
            }
        } else {
            tab = tab.color(Color::BrightBlack);
        }

        tabs = tabs.child(tab);
    }

    tabs.into()
}

/// Render content for the selected panel.
fn render_panel_content(panel: Panel, tier: RenderTier, phase: f32) -> Node {
    match panel {
        Panel::Visualization => render_visualization_panel(tier, phase),
        Panel::Code => render_code_panel(tier),
        Panel::Diff => render_diff_panel(tier),
        Panel::Chat => render_chat_panel(tier, phase),
    }
}

/// Render the visualization panel (Heatmap, Sparklines, Progress).
fn render_visualization_panel(tier: RenderTier, phase: f32) -> Node {
    // Generate animated data
    let heatmap_data = generate_heatmap_data(8, 12, phase);
    let cpu_data = generate_sparkline_data(24, phase, 1.2, 30.0);
    let mem_data = generate_sparkline_data(24, phase + 1.5, 0.8, 45.0);

    // Create components
    let heatmap = Heatmap::new(heatmap_data)
        .palette(HeatmapPalette::Viridis)
        .cell_width(2);

    let cpu_sparkline = Sparkline::new(cpu_data)
        .label("CPU")
        .color(Color::BrightGreen)
        .show_value(true)
        .max_width(24);

    let mem_sparkline = Sparkline::new(mem_data)
        .label("MEM")
        .color(Color::BrightBlue)
        .show_value(true)
        .max_width(24);

    let progress_value = (phase * 0.2).sin() * 0.5 + 0.5;
    let progress = Progress::new()
        .progress(progress_value)
        .label("Task")
        .width(30)
        .show_percentage(true)
        .filled_color(Color::BrightCyan);

    BoxNode::new()
        .flex_direction(FlexDirection::Row)
        .gap(4.0)
        .flex_grow(1.0)
        // Left panel: Heatmap
        .child(
            BoxNode::new()
                .flex_direction(FlexDirection::Column)
                .gap(1.0)
                .child(TextNode::new("Heatmap").bold().underline())
                .child(heatmap.render_for_tier(tier))
                .child(
                    TextNode::new(
                        heatmap
                            .tier_features()
                            .description_for_tier(tier)
                            .unwrap_or("Unknown tier"),
                    )
                    .italic()
                    .color(Color::BrightBlack),
                ),
        )
        // Right panel: Metrics
        .child(
            BoxNode::new()
                .flex_direction(FlexDirection::Column)
                .gap(1.0)
                .child(TextNode::new("Sparklines").bold().underline())
                .child(cpu_sparkline.render_for_tier(tier))
                .child(mem_sparkline.render_for_tier(tier))
                .child(Spacer::new())
                .child(TextNode::new("Progress").bold().underline())
                .child(progress.render_for_tier(tier)),
        )
        .into()
}

/// Render the code/markdown panel.
fn render_code_panel(tier: RenderTier) -> Node {
    let markdown = Markdown::new(SAMPLE_MARKDOWN);

    BoxNode::new()
        .flex_direction(FlexDirection::Column)
        .gap(1.0)
        .flex_grow(1.0)
        .child(TextNode::new("Markdown Component").bold().underline())
        .child(markdown.render_for_tier(tier))
        .child(
            TextNode::new(
                markdown
                    .tier_features()
                    .description_for_tier(tier)
                    .unwrap_or("Unknown tier"),
            )
            .italic()
            .color(Color::BrightBlack),
        )
        .into()
}

/// Render the diff panel.
fn render_diff_panel(tier: RenderTier) -> Node {
    // Create a sample diff
    let diff = DiffView::new()
        .file_path("src/main.rs")
        .line(DiffLine::context(1, "fn main() {"))
        .line(DiffLine::delete(2, "    println!(\"Hello\");"))
        .line(DiffLine::add(2, "    println!(\"Hello, World!\");"))
        .line(DiffLine::context(3, "}"))
        .line(DiffLine::hunk_separator())
        .line(DiffLine::context(10, "fn helper() {"))
        .line(DiffLine::add(11, "    // New comment"))
        .line(DiffLine::context(12, "    do_work();"))
        .line(DiffLine::delete(13, "    old_function();"))
        .line(DiffLine::add(13, "    new_function();"))
        .line(DiffLine::context(14, "}"))
        .show_line_numbers(true)
        .show_summary(true);

    BoxNode::new()
        .flex_direction(FlexDirection::Column)
        .gap(1.0)
        .flex_grow(1.0)
        .child(TextNode::new("DiffView Component").bold().underline())
        .child(diff.render_for_tier(tier))
        .child(
            TextNode::new(
                diff.tier_features()
                    .description_for_tier(tier)
                    .unwrap_or("Unknown tier"),
            )
            .italic()
            .color(Color::BrightBlack),
        )
        .into()
}

/// Render the chat/status panel.
fn render_chat_panel(tier: RenderTier, phase: f32) -> Node {
    // Create a sample chat conversation
    let chat = ChatView::new()
        .show_timestamps(true)
        .message(ChatMessage::new(MessageRole::User, "Hello! Can you help me?").timestamp("10:00"))
        .message(
            ChatMessage::new(
                MessageRole::Assistant,
                "**Of course!** I'm here to help.\n\nWhat would you like to know?",
            )
            .timestamp("10:01"),
        )
        .message(
            ChatMessage::new(MessageRole::User, "How does adaptive rendering work?")
                .timestamp("10:02"),
        )
        .message(
            ChatMessage::new(
                MessageRole::Assistant,
                "Adaptive rendering detects terminal capabilities and adjusts output:\n\n\
                - **Tier 0**: Plain text for dumb terminals\n\
                - **Tier 1**: ASCII art with basic colors\n\
                - **Tier 2**: Unicode with true color\n\
                - **Tier 3**: GPU acceleration",
            )
            .timestamp("10:03"),
        )
        .message(ChatMessage::new(MessageRole::System, "Connection stable").timestamp("10:04"));

    // Create animated status bars
    let frame = (phase * 10.0) as usize;
    let mut thinking_status = StatusBar::new()
        .state(StatusState::Thinking)
        .message("Analyzing request...");
    for _ in 0..frame {
        thinking_status.tick();
    }

    let mut executing_status = StatusBar::new()
        .state(StatusState::Executing)
        .message("Running build...");
    for _ in 0..frame {
        executing_status.tick();
    }

    let idle_status = StatusBar::new()
        .state(StatusState::Idle)
        .message("Ready for input");

    let error_status = StatusBar::new()
        .state(StatusState::Error)
        .message("Connection lost");

    BoxNode::new()
        .flex_direction(FlexDirection::Row)
        .gap(4.0)
        .flex_grow(1.0)
        // Left panel: Chat
        .child(
            BoxNode::new()
                .flex_direction(FlexDirection::Column)
                .gap(1.0)
                .flex_grow(1.0)
                .child(TextNode::new("ChatView Component").bold().underline())
                .child(chat.render_for_tier(tier))
                .child(
                    TextNode::new(
                        chat.tier_features()
                            .description_for_tier(tier)
                            .unwrap_or("Unknown tier"),
                    )
                    .italic()
                    .color(Color::BrightBlack),
                ),
        )
        // Right panel: Status bars
        .child(
            BoxNode::new()
                .flex_direction(FlexDirection::Column)
                .gap(1.0)
                .child(TextNode::new("StatusBar Component").bold().underline())
                .child(TextNode::new("Idle:"))
                .child(idle_status.render_for_tier(tier))
                .child(TextNode::new(""))
                .child(TextNode::new("Thinking (animated):"))
                .child(thinking_status.render_for_tier(tier))
                .child(TextNode::new(""))
                .child(TextNode::new("Executing (animated):"))
                .child(executing_status.render_for_tier(tier))
                .child(TextNode::new(""))
                .child(TextNode::new("Error:"))
                .child(error_status.render_for_tier(tier))
                .child(Spacer::new())
                .child(
                    TextNode::new(
                        StatusBar::new()
                            .tier_features()
                            .description_for_tier(tier)
                            .unwrap_or("Unknown tier"),
                    )
                    .italic()
                    .color(Color::BrightBlack),
                ),
        )
        .into()
}

/// Render a badge showing the current tier.
fn render_tier_badge(tier: RenderTier, auto_detect: bool) -> Node {
    let (name, color) = match tier {
        RenderTier::Tier0Fallback => ("T0: Fallback", Color::Red),
        RenderTier::Tier1Ansi => ("T1: ANSI", Color::Yellow),
        RenderTier::Tier2Retained => ("T2: Retained", Color::Green),
        RenderTier::Tier3Gpu => ("T3: GPU", Color::BrightGreen),
    };

    let label = if auto_detect {
        format!("[AUTO] {}", name)
    } else {
        format!("[MANUAL] {}", name)
    };

    TextNode::new(label).bold().color(color).into()
}

/// Render information about the current tier.
fn render_tier_info(tier: RenderTier) -> Node {
    let (icon, features) = match tier {
        RenderTier::Tier0Fallback => (
            "[!]",
            vec![
                "No colors or styling",
                "ASCII only (no Unicode)",
                "Text-based representations",
                "Safe for CI/logs",
            ],
        ),
        RenderTier::Tier1Ansi => (
            "[~]",
            vec![
                "256 colors",
                "Basic Unicode support",
                "ASCII art visualizations",
                "Most terminals",
            ],
        ),
        RenderTier::Tier2Retained => (
            "[*]",
            vec![
                "True color (24-bit RGB)",
                "Synchronized output (no tearing)",
                "Mouse support",
                "iTerm2, Kitty, etc.",
            ],
        ),
        RenderTier::Tier3Gpu => (
            "[#]",
            vec![
                "GPU acceleration",
                "120 FPS capable",
                "Shader-based rendering",
                "dashterm2/dterm",
            ],
        ),
    };

    let color = match tier {
        RenderTier::Tier0Fallback => Color::Red,
        RenderTier::Tier1Ansi => Color::Yellow,
        RenderTier::Tier2Retained => Color::Green,
        RenderTier::Tier3Gpu => Color::BrightGreen,
    };

    BoxNode::new()
        .flex_direction(FlexDirection::Column)
        .padding(Edges::new(1.0, 0.0, 0.0, 0.0))
        .child(
            BoxNode::new()
                .flex_direction(FlexDirection::Row)
                .gap(1.0)
                .child(TextNode::new(icon).color(color))
                .child(TextNode::new(format!("{} Features:", tier.name())).bold()),
        )
        .child(features.iter().fold(
            BoxNode::new().flex_direction(FlexDirection::Column),
            |container, &feature| {
                container.child(TextNode::new(format!("  - {}", feature)).color(Color::BrightBlack))
            },
        ))
        .into()
}
