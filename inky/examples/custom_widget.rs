//! Custom widget example demonstrating the Widget trait and CustomNode.
#![allow(clippy::format_push_string, clippy::many_single_char_names)] // Example code prioritizes readability
//!
//! This example shows how to create custom widgets that integrate with
//! inky's layout and rendering system. Custom widgets enable:
//! - Porting complex existing UIs with custom rendering logic
//! - Creating specialized visualizations
//! - Direct buffer access for maximum control
//!
//! # Painter API Quick Reference
//!
//! The `Painter` provides access to the underlying buffer for direct drawing:
//!
//! ```ignore
//! impl Widget for MyWidget {
//!     fn render(&self, ctx: &WidgetContext, painter: &mut Painter) {
//!         let buf = painter.buffer_mut();
//!
//!         // Q1: Draw single character at position
//!         let mut cell = Cell::new('X');
//!         cell.set_fg(PackedColor::from(Color::Red));
//!         buf.set(x, y, cell);
//!
//!         // Q2: Draw horizontal/vertical lines
//!         for dx in 0..width {
//!             let mut cell = Cell::new('─');
//!             cell.set_fg(PackedColor::from(Color::Blue));
//!             buf.set(x + dx, y, cell);
//!         }
//!
//!         // Q3: Fill rectangle
//!         let fill_cell = Cell::new(' ').with_bg(Color::Green);
//!         buf.fill(x, y, width, height, fill_cell);
//!
//!         // Q4: Draw styled text at position
//!         buf.write_str(x, y, "Hello", Color::Yellow, Color::Default);
//!     }
//! }
//! ```
//!
//! See `PainterDemoWidget` below for a complete working example.
//!
//! Run with:
//! ```bash
//! cargo run --example custom_widget
//! ```
//!
//! Controls:
//! - Up/Down: Change fill percentage
//! - Left/Right: Change bar style
//! - Space: Toggle animation
//! - q / Escape: Quit

use inky::node::{CustomNode, Widget, WidgetContext};
use inky::prelude::*;
use inky::render::{Cell, CellFlags, PackedColor, Painter};
use std::time::Duration;

// ============================================================================
// PainterDemoWidget - Demonstrates all Painter API patterns
// ============================================================================

/// Demonstrates all four Painter API patterns requested by the Codex porter.
///
/// This widget shows how to:
/// 1. Draw single characters at positions
/// 2. Draw horizontal and vertical lines
/// 3. Fill rectangles with background colors
/// 4. Draw styled text at specific positions
struct PainterDemoWidget {
    /// Demo title.
    title: String,
}

impl PainterDemoWidget {
    fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
        }
    }
}

impl Widget for PainterDemoWidget {
    fn render(&self, ctx: &WidgetContext, painter: &mut Painter) {
        let buf = painter.buffer_mut();

        // Minimum size check
        if ctx.width < 30 || ctx.height < 8 {
            return;
        }

        // ----------------------------------------------------------------
        // Pattern 1: Draw single character at position
        // ----------------------------------------------------------------
        // Draw corner markers
        let corner_color = PackedColor::from(Color::BrightYellow);
        let mut corner = Cell::new('◆');
        corner.set_fg(corner_color);
        buf.set(ctx.x, ctx.y, corner);
        buf.set(ctx.x + ctx.width - 1, ctx.y, corner);
        buf.set(ctx.x, ctx.y + ctx.height - 1, corner);
        buf.set(ctx.x + ctx.width - 1, ctx.y + ctx.height - 1, corner);

        // ----------------------------------------------------------------
        // Pattern 2: Draw horizontal and vertical lines
        // ----------------------------------------------------------------
        let line_color = PackedColor::from(Color::BrightBlue);

        // Horizontal line at top (between corners)
        for dx in 1..ctx.width - 1 {
            let mut cell = Cell::new('─');
            cell.set_fg(line_color);
            buf.set(ctx.x + dx, ctx.y, cell);
        }

        // Horizontal line at bottom
        for dx in 1..ctx.width - 1 {
            let mut cell = Cell::new('─');
            cell.set_fg(line_color);
            buf.set(ctx.x + dx, ctx.y + ctx.height - 1, cell);
        }

        // Vertical lines on left and right
        for dy in 1..ctx.height - 1 {
            let mut cell = Cell::new('│');
            cell.set_fg(line_color);
            buf.set(ctx.x, ctx.y + dy, cell);
            buf.set(ctx.x + ctx.width - 1, ctx.y + dy, cell);
        }

        // ----------------------------------------------------------------
        // Pattern 3: Fill rectangle with background color
        // ----------------------------------------------------------------
        // Fill interior with a subtle background
        let fill_cell = Cell::blank().with_bg(Color::Rgb(30, 30, 50));
        buf.fill(
            ctx.x + 1,
            ctx.y + 1,
            ctx.width - 2,
            ctx.height - 2,
            fill_cell,
        );

        // Draw a small colored box inside
        if ctx.height > 4 && ctx.width > 10 {
            let box_cell = Cell::new(' ').with_bg(Color::Rgb(80, 120, 80));
            buf.fill(ctx.x + 2, ctx.y + 2, 6, 2, box_cell);
        }

        // ----------------------------------------------------------------
        // Pattern 4: Draw styled text at position
        // ----------------------------------------------------------------
        // Title text (with Buffer::write_str)
        let title_x = ctx.x + 2;
        let title_y = ctx.y + 1;
        if title_y < ctx.y + ctx.height - 1 {
            buf.write_str(
                title_x,
                title_y,
                &self.title,
                Color::BrightWhite,
                Color::Default,
            );
        }

        // Styled text using Cell for more control (bold + color)
        let info_y = ctx.y + ctx.height - 2;
        if info_y > ctx.y + 1 {
            let info = "Painter API Demo";
            for (i, ch) in info.chars().enumerate() {
                let x = ctx.x + 2 + i as u16;
                if x >= ctx.x + ctx.width - 1 {
                    break;
                }
                let mut cell = Cell::new(ch);
                cell.set_fg(PackedColor::from(Color::BrightCyan));
                cell.flags |= CellFlags::BOLD;
                buf.set(x, info_y, cell);
            }
        }
    }

    fn measure(&self, available_width: u16, _available_height: u16) -> (u16, u16) {
        // Request minimum 30x5, but take available width
        (available_width.max(30), 5)
    }
}

// ============================================================================

/// Different visual styles for the progress bar.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BarStyle {
    Blocks,
    Gradient,
    Ascii,
    Dots,
}

impl BarStyle {
    fn next(self) -> Self {
        match self {
            BarStyle::Blocks => BarStyle::Gradient,
            BarStyle::Gradient => BarStyle::Ascii,
            BarStyle::Ascii => BarStyle::Dots,
            BarStyle::Dots => BarStyle::Blocks,
        }
    }

    fn prev(self) -> Self {
        match self {
            BarStyle::Blocks => BarStyle::Dots,
            BarStyle::Gradient => BarStyle::Blocks,
            BarStyle::Ascii => BarStyle::Gradient,
            BarStyle::Dots => BarStyle::Ascii,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            BarStyle::Blocks => "Blocks",
            BarStyle::Gradient => "Gradient",
            BarStyle::Ascii => "ASCII",
            BarStyle::Dots => "Dots",
        }
    }
}

/// A custom progress bar widget with multiple visual styles.
///
/// This demonstrates implementing the `Widget` trait for custom rendering.
struct FancyProgressBar {
    /// Progress value from 0.0 to 1.0.
    progress: f32,
    /// Visual style.
    style: BarStyle,
    /// Bar label.
    label: String,
    /// Foreground color.
    fg_color: Color,
    /// Background color.
    bg_color: Color,
}

impl FancyProgressBar {
    fn new(progress: f32, style: BarStyle, label: impl Into<String>) -> Self {
        Self {
            progress: progress.clamp(0.0, 1.0),
            style,
            label: label.into(),
            fg_color: Color::BrightCyan,
            bg_color: Color::BrightBlack,
        }
    }

    /// Get the fill characters for the current style.
    fn fill_chars(&self) -> (&str, &str) {
        match self.style {
            BarStyle::Blocks => ("█", "░"),
            BarStyle::Gradient => ("▓", "░"),
            BarStyle::Ascii => ("#", "-"),
            BarStyle::Dots => ("●", "○"),
        }
    }
}

impl Widget for FancyProgressBar {
    /// Render the widget to the buffer using the Painter API.
    fn render(&self, ctx: &WidgetContext, painter: &mut Painter) {
        let (filled_char, empty_char) = self.fill_chars();

        // Reserve space for label and percentage
        let label_width = self.label.len() as u16 + 2; // "Label: "
        let percent_width = 5u16; // " 100%"
        let bar_width = ctx.width.saturating_sub(label_width + percent_width);

        if bar_width < 3 || ctx.height < 1 {
            return;
        }

        // Calculate filled portion
        let filled = ((bar_width as f32) * self.progress).round() as u16;
        let empty = bar_width - filled;

        // Build the bar string
        let mut bar = String::with_capacity(ctx.width as usize);
        bar.push_str(&self.label);
        bar.push_str(": ");

        for _ in 0..filled {
            bar.push_str(filled_char);
        }
        for _ in 0..empty {
            bar.push_str(empty_char);
        }

        // Add percentage
        bar.push_str(&format!(" {:3.0}%", self.progress * 100.0));

        // Render using the painter's underlying buffer
        let buf = painter.buffer_mut();
        let y = ctx.y;

        for (i, ch) in bar.chars().enumerate() {
            let x = ctx.x + i as u16;
            if x >= ctx.x + ctx.width {
                break;
            }

            // Color the filled portion differently
            let in_bar = i >= self.label.len() + 2;
            let in_filled = in_bar && i < self.label.len() + 2 + filled as usize;

            let mut cell = inky::render::Cell::new(ch);
            if in_filled {
                cell.set_fg(inky::render::PackedColor::from(self.fg_color));
            } else if in_bar {
                cell.set_fg(inky::render::PackedColor::from(self.bg_color));
            } else {
                cell.set_fg(inky::render::PackedColor::from(Color::White));
            }
            buf.set(x, y, cell);
        }
    }

    /// Return the preferred size for this widget.
    fn measure(&self, available_width: u16, _available_height: u16) -> (u16, u16) {
        // We need at least: label + ": " + bar (min 10) + " 100%"
        let min_width = self.label.len() as u16 + 2 + 10 + 5;
        let width = available_width.max(min_width);
        (width, 1) // Single-row widget
    }
}

/// A custom gauge widget showing a circular/semicircular visualization.
struct GaugeWidget {
    value: f32,
    label: String,
}

impl GaugeWidget {
    fn new(value: f32, label: impl Into<String>) -> Self {
        Self {
            value: value.clamp(0.0, 1.0),
            label: label.into(),
        }
    }
}

impl Widget for GaugeWidget {
    fn render(&self, ctx: &WidgetContext, painter: &mut Painter) {
        if ctx.width < 7 || ctx.height < 3 {
            return;
        }

        let buf = painter.buffer_mut();

        // Draw a simple ASCII gauge
        // Row 0: top arc
        // Row 1: value display
        // Row 2: label

        // Draw gauge arc using Unicode box drawing
        let segments = 7;
        let filled = ((segments as f32) * self.value).round() as usize;

        // Color based on value
        let color = if self.value < 0.3 {
            Color::Red
        } else if self.value < 0.7 {
            Color::Yellow
        } else {
            Color::Green
        };

        // Row 0: Arc top
        let arc_chars = ['╭', '─', '─', '─', '─', '─', '╮'];
        let start_x = ctx.x + (ctx.width.saturating_sub(7)) / 2;
        for (i, ch) in arc_chars.iter().enumerate() {
            let x = start_x + i as u16;
            if x < ctx.x + ctx.width {
                let mut cell = inky::render::Cell::new(*ch);
                if i > 0 && i < arc_chars.len() - 1 && i <= filled {
                    cell.set_fg(inky::render::PackedColor::from(color));
                } else {
                    cell.set_fg(inky::render::PackedColor::from(Color::BrightBlack));
                }
                buf.set(x, ctx.y, cell);
            }
        }

        // Row 1: Value percentage centered
        let percent = format!("{:3.0}%", self.value * 100.0);
        let percent_x = ctx.x + (ctx.width.saturating_sub(percent.len() as u16)) / 2;
        for (i, ch) in percent.chars().enumerate() {
            let x = percent_x + i as u16;
            if x < ctx.x + ctx.width && ctx.y + 1 < ctx.y + ctx.height {
                let mut cell = inky::render::Cell::new(ch);
                cell.set_fg(inky::render::PackedColor::from(color));
                cell.flags |= inky::render::CellFlags::BOLD;
                buf.set(x, ctx.y + 1, cell);
            }
        }

        // Row 2: Label centered
        let label_x = ctx.x + (ctx.width.saturating_sub(self.label.len() as u16)) / 2;
        for (i, ch) in self.label.chars().enumerate() {
            let x = label_x + i as u16;
            if x < ctx.x + ctx.width && ctx.y + 2 < ctx.y + ctx.height {
                let mut cell = inky::render::Cell::new(ch);
                cell.set_fg(inky::render::PackedColor::from(Color::White));
                buf.set(x, ctx.y + 2, cell);
            }
        }
    }

    fn measure(&self, available_width: u16, _available_height: u16) -> (u16, u16) {
        let width = available_width.max(9);
        (width, 3) // 3 rows: arc, value, label
    }
}

/// Application state.
struct AppState {
    /// Current progress value.
    progress: Signal<f32>,
    /// Current bar style.
    bar_style: Signal<BarStyle>,
    /// Animation enabled.
    animating: Signal<bool>,
    /// Animation timer.
    timer: IntervalHandle,
    /// Animation direction.
    direction: Signal<f32>,
}

impl AppState {
    fn new() -> Self {
        Self {
            progress: use_signal(0.42),
            bar_style: use_signal(BarStyle::Blocks),
            animating: use_signal(false),
            timer: use_interval(Duration::from_millis(50)),
            direction: use_signal(0.01),
        }
    }
}

fn main() -> Result<()> {
    let state = AppState::new();

    App::new()
        .state(state)
        .alt_screen(true)
        .on_key(|state, event| {
            match event.code {
                KeyCode::Char('q') | KeyCode::Esc => return true,
                KeyCode::Up => state.progress.update(|p| *p = (*p + 0.05).min(1.0)),
                KeyCode::Down => state.progress.update(|p| *p = (*p - 0.05).max(0.0)),
                KeyCode::Right => state.bar_style.update(|s| *s = s.next()),
                KeyCode::Left => state.bar_style.update(|s| *s = s.prev()),
                KeyCode::Char(' ') => state.animating.update(|a| *a = !*a),
                _ => {}
            }
            false
        })
        .render(|ctx| {
            let state = ctx.state;
            let progress = state.progress.get();
            let bar_style = state.bar_style.get();
            let animating = state.animating.get();

            // Handle animation via timer (tick count increases each interval)
            let _tick = state.timer.get(); // Access triggers re-render on interval
            if animating {
                let dir = state.direction.get();
                let new_progress = progress + dir;
                if new_progress >= 1.0 {
                    state.progress.set(1.0);
                    state.direction.set(-0.01);
                } else if new_progress <= 0.0 {
                    state.progress.set(0.0);
                    state.direction.set(0.01);
                } else {
                    state.progress.set(new_progress);
                }
            }

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
                        .child(
                            TextNode::new("Custom Widget Demo")
                                .color(Color::BrightCyan)
                                .bold(),
                        ),
                )
                .child(
                    // Description
                    BoxNode::new()
                        .width(Dimension::Percent(100.0))
                        .margin_xy(0.0, 1.0)
                        .child(
                            TextNode::new(
                                "This example demonstrates the Widget trait for custom rendering.",
                            )
                            .color(Color::BrightBlack),
                        ),
                )
                .child(
                    // Custom progress bars using Widget trait
                    BoxNode::new()
                        .width(Dimension::Percent(100.0))
                        .flex_direction(FlexDirection::Column)
                        .border(BorderStyle::Single)
                        .padding(1)
                        .margin_xy(0.0, 1.0)
                        .child(
                            TextNode::new("FancyProgressBar (Custom Widget)")
                                .color(Color::Yellow)
                                .bold(),
                        )
                        .child(BoxNode::new().height(1)) // Spacer
                        .child(
                            // First custom widget - uses current style
                            CustomNode::new(FancyProgressBar::new(
                                progress,
                                bar_style,
                                format!("Style: {}", bar_style.name()),
                            ))
                            .width(Dimension::Percent(100.0)),
                        )
                        .child(BoxNode::new().height(1)) // Spacer
                        .child(
                            // Second custom widget - different style
                            CustomNode::new(FancyProgressBar::new(
                                progress * 0.7,
                                BarStyle::Gradient,
                                "Secondary",
                            ))
                            .width(Dimension::Percent(100.0)),
                        ),
                )
                .child(
                    // Gauge widgets
                    BoxNode::new()
                        .width(Dimension::Percent(100.0))
                        .flex_direction(FlexDirection::Row)
                        .border(BorderStyle::Single)
                        .padding(1)
                        .margin_xy(0.0, 1.0)
                        .child(
                            BoxNode::new()
                                .flex_grow(1.0)
                                .child(
                                    CustomNode::new(GaugeWidget::new(progress, "CPU"))
                                        .width(Dimension::Percent(100.0)),
                                ),
                        )
                        .child(
                            BoxNode::new()
                                .flex_grow(1.0)
                                .child(
                                    CustomNode::new(GaugeWidget::new(progress * 0.8, "Memory"))
                                        .width(Dimension::Percent(100.0)),
                                ),
                        )
                        .child(
                            BoxNode::new()
                                .flex_grow(1.0)
                                .child(
                                    CustomNode::new(GaugeWidget::new(
                                        (progress * 2.0) % 1.0,
                                        "Disk",
                                    ))
                                    .width(Dimension::Percent(100.0)),
                                ),
                        ),
                )
                .child(
                    // Painter API Demo widget - demonstrates all 4 patterns
                    BoxNode::new()
                        .width(Dimension::Percent(100.0))
                        .margin_xy(0.0, 1.0)
                        .child(
                            CustomNode::new(PainterDemoWidget::new("All 4 Painter Patterns"))
                                .width(Dimension::Percent(100.0))
                                .height(8),
                        ),
                )
                .child(Spacer::new())
                .child(
                    // Controls
                    BoxNode::new()
                        .width(Dimension::Percent(100.0))
                        .flex_direction(FlexDirection::Row)
                        .child(
                            TextNode::new(format!(
                                "[Up/Down] Value: {:.0}%  [Left/Right] Style: {}  [Space] {}  [q] Quit",
                                progress * 100.0,
                                bar_style.name(),
                                if animating { "Stop" } else { "Animate" }
                            ))
                            .color(Color::BrightBlack),
                        ),
                )
                .into()
        })
        .run()?;

    Ok(())
}
