//! Ratatui benchmark implementations.
//!
//! These functions implement the standard benchmark scenarios using ratatui.
//! Only compiled when the `compat-ratatui` feature is enabled.

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use super::scenarios::{generate_grid_text, generate_messages};

/// Create an empty buffer (startup cost).
pub fn create_empty_buffer(width: u16, height: u16) -> Buffer {
    Buffer::empty(Rect::new(0, 0, width, height))
}

/// Render a text grid (layout + render).
pub fn render_text_grid(rows: usize, cols: usize) -> Buffer {
    let grid = generate_grid_text(rows, cols);
    let area = Rect::new(0, 0, 200, 50);
    let mut buffer = Buffer::empty(area);

    // Calculate row height
    let row_height = area.height / rows as u16;

    for (r, row) in grid.iter().enumerate() {
        let row_area = Rect::new(0, r as u16 * row_height, area.width, row_height);
        let col_width = row_area.width / cols as u16;

        for (c, text) in row.iter().enumerate() {
            let cell_area = Rect::new(
                row_area.x + c as u16 * col_width,
                row_area.y,
                col_width,
                row_height,
            );
            Paragraph::new(text.as_str()).render(cell_area, &mut buffer);
        }
    }

    buffer
}

/// Render a chat UI (realistic app scenario).
pub fn render_chat_ui(message_count: usize) -> Buffer {
    let messages = generate_messages(message_count);
    let area = Rect::new(0, 0, 120, 40);
    let mut buffer = Buffer::empty(area);

    // Layout: header (1), messages (flexible), input (3)
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

    // Header
    Paragraph::new("Chat with Claude")
        .style(Style::default())
        .render(layout[0], &mut buffer);

    // Messages
    let msg_lines: Vec<Line> = messages
        .iter()
        .flat_map(|(role, content)| {
            vec![
                Line::from(format!("{}:", role)),
                Line::from(content.as_str()),
            ]
        })
        .collect();
    Paragraph::new(Text::from(msg_lines)).render(layout[1], &mut buffer);

    // Input
    Paragraph::new("> Type your message here...")
        .block(Block::default().borders(Borders::TOP))
        .render(layout[2], &mut buffer);

    buffer
}

/// Full screen redraw (worst case).
pub fn full_redraw(width: u16, height: u16) -> Buffer {
    let area = Rect::new(0, 0, width, height);
    let mut buffer = Buffer::empty(area);

    // Create lines that fill the entire screen
    let lines: Vec<Line> = (0..height)
        .map(|i| {
            Line::from(format!(
                "{:width$}",
                format!("Line {} with content that fills the row", i),
                width = width as usize
            ))
        })
        .collect();

    Paragraph::new(Text::from(lines)).render(area, &mut buffer);
    buffer
}
