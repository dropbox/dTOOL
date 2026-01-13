//! Ratatui to Inky Migration Guide
#![allow(dead_code)] // Some patterns shown but not displayed in demo
//!
//! This example demonstrates how to port common ratatui patterns to inky.
//! Use this as a reference when migrating existing ratatui-based TUIs.
//!
//! Quick equivalences:
//! ```text
//! ratatui: Paragraph::new(text).style(style)
//! inky:    TextNode::new(text).color(...).bold()
//! ratatui: frame.render_widget(widget, area)
//! inky:    CustomNode::new(widget)
//! ratatui: Line::from(vec![Span::styled(...)])
//! inky:    TextNode::from_spans(vec![StyledSpan::new(...)])
//! ```
//!
//! Run with:
//! ```bash
//! cargo run --example ratatui_migration
//! ```

use inky::node::{BoxNode, CustomNode, TextNode, Widget, WidgetContext};
use inky::prelude::*;
use inky::render::Painter;

// =============================================================================
// PATTERN 1: Basic Text
// =============================================================================
//
// ratatui:
//   Paragraph::new("Hello, World!")
//
// inky:
//   TextNode::new("Hello, World!")

fn basic_text() -> Node {
    TextNode::new("Hello, World!").into()
}

// =============================================================================
// PATTERN 2: Styled Text
// =============================================================================
//
// ratatui:
//   Paragraph::new("Bold text")
//       .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
//
// inky:
//   TextNode::new("Bold text").color(Color::Cyan).bold()

fn styled_text() -> Node {
    TextNode::new("Bold cyan text")
        .color(Color::Cyan)
        .bold()
        .into()
}

// =============================================================================
// PATTERN 3: Multiple Styled Spans (Line/Spans)
// =============================================================================
//
// ratatui:
//   Line::from(vec![
//       Span::raw("Normal "),
//       Span::styled("bold", Style::default().add_modifier(Modifier::BOLD)),
//       Span::raw(" and "),
//       Span::styled("red", Style::default().fg(Color::Red)),
//   ])
//
// inky:
//   TextNode::from_spans(vec![
//       StyledSpan::new("Normal "),
//       StyledSpan::new("bold").bold(),
//       StyledSpan::new(" and "),
//       StyledSpan::new("red").color(Color::Red),
//   ])

fn styled_spans() -> Node {
    TextNode::from_spans(vec![
        StyledSpan::new("Normal "),
        StyledSpan::new("bold").bold(),
        StyledSpan::new(" and "),
        StyledSpan::new("red").color(Color::Red),
    ])
    .into()
}

// =============================================================================
// PATTERN 4: ANSI Text Passthrough
// =============================================================================
//
// ratatui (with ansi-to-tui crate):
//   let text = ansi_to_tui::IntoText::into_text(&ansi_string)?;
//   Paragraph::new(text)
//
// inky (built-in):
//   TextNode::from_ansi(&ansi_string)

fn ansi_text() -> Node {
    let ansi_string = "\x1b[31mRed\x1b[0m \x1b[32mGreen\x1b[0m \x1b[1;34mBold Blue\x1b[0m";
    TextNode::from_ansi(ansi_string).into()
}

// =============================================================================
// PATTERN 5: Vertical Layout (Column)
// =============================================================================
//
// ratatui:
//   let chunks = Layout::default()
//       .direction(Direction::Vertical)
//       .constraints([Constraint::Length(1), Constraint::Min(0)])
//       .split(area);
//   frame.render_widget(header, chunks[0]);
//   frame.render_widget(content, chunks[1]);
//
// inky:
//   BoxNode::new()
//       .flex_direction(FlexDirection::Column)
//       .child(header)
//       .child(content)

fn vertical_layout() -> Node {
    BoxNode::new()
        .flex_direction(FlexDirection::Column)
        .child(TextNode::new("Header").bold())
        .child(TextNode::new("Content area"))
        .into()
}

// =============================================================================
// PATTERN 6: Horizontal Layout (Row)
// =============================================================================
//
// ratatui:
//   let chunks = Layout::default()
//       .direction(Direction::Horizontal)
//       .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
//       .split(area);
//
// inky:
//   BoxNode::new()
//       .flex_direction(FlexDirection::Row)
//       .child(BoxNode::new().flex_grow(1.0).child(left))
//       .child(BoxNode::new().flex_grow(1.0).child(right))

fn horizontal_layout() -> Node {
    BoxNode::new()
        .flex_direction(FlexDirection::Row)
        .child(
            BoxNode::new()
                .flex_grow(1.0)
                .child(TextNode::new("Left panel")),
        )
        .child(
            BoxNode::new()
                .flex_grow(1.0)
                .child(TextNode::new("Right panel")),
        )
        .into()
}

// =============================================================================
// PATTERN 7: Block with Border
// =============================================================================
//
// ratatui:
//   Block::default()
//       .title("Title")
//       .borders(Borders::ALL)
//       .border_type(BorderType::Rounded)
//
// inky:
//   BoxNode::new()
//       .border(BorderStyle::Rounded)
//       .child(TextNode::new("Title").bold())
//       .child(content)

fn bordered_block() -> Node {
    BoxNode::new()
        .border(BorderStyle::Rounded)
        .padding(1)
        .flex_direction(FlexDirection::Column)
        .child(TextNode::new("Title").bold().color(Color::Cyan))
        .child(TextNode::new("Content inside bordered block"))
        .into()
}

// =============================================================================
// PATTERN 8: Fixed Size Elements
// =============================================================================
//
// ratatui:
//   Constraint::Length(3)  // Fixed 3 rows
//   Constraint::Min(5)     // At least 5 rows
//
// inky:
//   BoxNode::new().height(3)           // Fixed height
//   BoxNode::new().min_height(5)       // Minimum height

fn fixed_sizes() -> Node {
    BoxNode::new()
        .flex_direction(FlexDirection::Column)
        .child(
            BoxNode::new()
                .height(3)
                .border(BorderStyle::Single)
                .child(TextNode::new("Fixed 3 rows")),
        )
        .child(
            BoxNode::new()
                .flex_grow(1.0)
                .border(BorderStyle::Single)
                .child(TextNode::new("Flexible height (grows)")),
        )
        .into()
}

// =============================================================================
// PATTERN 9: Custom Widget
// =============================================================================
//
// ratatui:
//   impl Widget for MyWidget {
//       fn render(self, area: Rect, buf: &mut Buffer) { ... }
//   }
//   frame.render_widget(my_widget, area);
//
// inky:
//   impl Widget for MyWidget {
//       fn render(&self, ctx: &WidgetContext, painter: &mut Painter) { ... }
//       fn measure(&self, w: u16, h: u16) -> (u16, u16) { ... }
//   }
//   CustomNode::new(my_widget)

/// Example custom widget - a simple horizontal bar
struct HorizontalBar {
    progress: f32,
    color: Color,
}

impl HorizontalBar {
    fn new(progress: f32, color: Color) -> Self {
        Self {
            progress: progress.clamp(0.0, 1.0),
            color,
        }
    }
}

impl Widget for HorizontalBar {
    fn render(&self, ctx: &WidgetContext, painter: &mut Painter) {
        let filled = ((ctx.width as f32) * self.progress) as u16;
        let buf = painter.buffer_mut();

        for x in 0..ctx.width {
            let ch = if x < filled { '█' } else { '░' };
            let mut cell = inky::render::Cell::new(ch);
            if x < filled {
                cell.set_fg(inky::render::PackedColor::from(self.color));
            } else {
                cell.set_fg(inky::render::PackedColor::from(Color::BrightBlack));
            }
            buf.set(ctx.x + x, ctx.y, cell);
        }
    }

    fn measure(&self, available_width: u16, _available_height: u16) -> (u16, u16) {
        (available_width.min(40), 1)
    }
}

fn custom_widget() -> Node {
    BoxNode::new()
        .flex_direction(FlexDirection::Column)
        .child(TextNode::new("Custom Widget (progress bar):"))
        .child(
            CustomNode::new(HorizontalBar::new(0.7, Color::Green)).width(Dimension::Percent(100.0)),
        )
        .into()
}

// =============================================================================
// PATTERN 10: Stateful Widget (ratatui StatefulWidget)
// =============================================================================
//
// ratatui:
//   impl StatefulWidget for MyList {
//       type State = ListState;
//       fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) { ... }
//   }
//
// inky:
//   Use Signal<T> for state, access via closure:
//   let selected = use_signal(0usize);
//   Select::new(items).selected(selected.get())

fn stateful_widget_equivalent() -> Node {
    // In real usage, you'd use:
    // let selected = use_signal(0usize);
    // Select::new(items).selected(selected.get())
    TextNode::new("Use Signal<T> for stateful widgets - see examples/focus.rs").into()
}

// =============================================================================
// PATTERN 11: Scrollable Content
// =============================================================================
//
// ratatui:
//   Paragraph::new(long_text).scroll((offset, 0))
//
// inky:
//   Scroll::new()
//       .offset(offset)
//       .child(TextNode::new(long_text))

fn scrollable_content() -> Node {
    let long_text = (0..20)
        .map(|i| format!("Line {}", i))
        .collect::<Vec<_>>()
        .join("\n");

    BoxNode::new()
        .height(5)
        .border(BorderStyle::Single)
        .child(
            Scroll::new()
                .offset_y(0) // Would be Signal in real app
                .child(TextNode::new(long_text)),
        )
        .into()
}

// =============================================================================
// PATTERN 12: Centering Content
// =============================================================================
//
// ratatui:
//   let centered = Layout::default()
//       .direction(Direction::Vertical)
//       .constraints([Constraint::Fill(1), Constraint::Length(3), Constraint::Fill(1)])
//       .split(area)[1];
//
// inky:
//   BoxNode::new()
//       .justify_content(JustifyContent::Center)
//       .align_items(AlignItems::Center)
//       .child(content)

fn centered_content() -> Node {
    BoxNode::new()
        .height(5)
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .border(BorderStyle::Rounded)
        .child(TextNode::new("Centered!").bold())
        .into()
}

// =============================================================================
// MAIN: Demonstrate all patterns
// =============================================================================

fn main() -> Result<()> {
    App::new()
        .alt_screen(true)
        .on_key(|_state, event| matches!(event.code, KeyCode::Char('q') | KeyCode::Esc))
        .render(|ctx| {
            BoxNode::new()
                .width(ctx.width())
                .height(ctx.height())
                .flex_direction(FlexDirection::Column)
                .padding(1)
                .child(
                    BoxNode::new()
                        .border(BorderStyle::Double)
                        .padding_xy(1.0, 0.0)
                        .child(
                            TextNode::new("Ratatui to Inky Migration Guide")
                                .color(Color::BrightCyan)
                                .bold(),
                        ),
                )
                .child(BoxNode::new().height(1)) // Spacer
                .child(
                    BoxNode::new()
                        .flex_grow(1.0)
                        .flex_direction(FlexDirection::Row)
                        .gap(2.0)
                        // Left column
                        .child(
                            BoxNode::new()
                                .flex_grow(1.0)
                                .flex_direction(FlexDirection::Column)
                                .gap(1.0)
                                .child(section("1. Basic Text", basic_text()))
                                .child(section("2. Styled Text", styled_text()))
                                .child(section("3. Styled Spans", styled_spans()))
                                .child(section("4. ANSI Passthrough", ansi_text()))
                                .child(section("5. Vertical Layout", vertical_layout()))
                                .child(section("6. Horizontal Layout", horizontal_layout())),
                        )
                        // Right column
                        .child(
                            BoxNode::new()
                                .flex_grow(1.0)
                                .flex_direction(FlexDirection::Column)
                                .gap(1.0)
                                .child(section("7. Bordered Block", bordered_block()))
                                .child(section("9. Custom Widget", custom_widget()))
                                .child(section("11. Scrollable", scrollable_content()))
                                .child(section("12. Centered", centered_content())),
                        ),
                )
                .child(
                    TextNode::new("Press 'q' to quit | See source for ratatui equivalents")
                        .color(Color::BrightBlack),
                )
                .into()
        })
        .run()?;

    Ok(())
}

fn section(title: &str, content: Node) -> Node {
    BoxNode::new()
        .flex_direction(FlexDirection::Column)
        .child(TextNode::new(title).color(Color::Yellow).bold())
        .child(content)
        .into()
}
