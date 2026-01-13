//! Widget showcase example demonstrating all Phase 3 components.
//!
//! Displays Input, Select, Progress, Spinner, Scroll, and Stack components.
//! Use arrow keys to navigate, Tab to switch components, 'q' to quit.

use inky::prelude::*;

/// Which widget is currently focused
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusedWidget {
    Input,
    Select,
    Progress,
    Scroll,
}

impl FocusedWidget {
    fn next(self) -> Self {
        match self {
            FocusedWidget::Input => FocusedWidget::Select,
            FocusedWidget::Select => FocusedWidget::Progress,
            FocusedWidget::Progress => FocusedWidget::Scroll,
            FocusedWidget::Scroll => FocusedWidget::Input,
        }
    }

    fn prev(self) -> Self {
        match self {
            FocusedWidget::Input => FocusedWidget::Scroll,
            FocusedWidget::Select => FocusedWidget::Input,
            FocusedWidget::Progress => FocusedWidget::Select,
            FocusedWidget::Scroll => FocusedWidget::Progress,
        }
    }
}

/// Application state
struct State {
    focused: FocusedWidget,
    input: Input,
    select_index: usize,
    progress: f32,
    scroll_offset: u16,
    spinner_frame: usize,
}

impl Default for State {
    fn default() -> Self {
        Self {
            focused: FocusedWidget::Input,
            input: Input::new().placeholder("Type something...").width(30),
            select_index: 0,
            progress: 0.45,
            scroll_offset: 0,
            spinner_frame: 0,
        }
    }
}

fn main() -> Result<()> {
    let select_options = vec![
        "Option 1: Apple",
        "Option 2: Banana",
        "Option 3: Cherry",
        "Option 4: Date",
        "Option 5: Elderberry",
    ];

    let scroll_lines: Vec<String> = (1..=30)
        .map(|i| format!("Scroll line {}: Lorem ipsum dolor sit amet", i))
        .collect();

    App::new()
        .state(State::default())
        .alt_screen(true)
        .render(move |ctx| {
            let state = ctx.state;

            // Main container
            BoxNode::new()
                .width(ctx.width())
                .height(ctx.height())
                .flex_direction(FlexDirection::Column)
                .padding(1)
                .child(
                    // Title
                    BoxNode::new()
                        .justify_content(JustifyContent::Center)
                        .child(
                            TextNode::new("inky Widget Showcase")
                                .color(Color::BrightCyan)
                                .bold(),
                        ),
                )
                .child(
                    TextNode::new("Tab: switch focus | Arrow keys: interact | q: quit")
                        .color(Color::BrightBlack),
                )
                .child(Spacer::new())
                .child(
                    // Two columns
                    BoxNode::new()
                        .flex_direction(FlexDirection::Row)
                        .gap(4.0)
                        .flex_grow(1.0)
                        .child(
                            // Left column
                            BoxNode::new()
                                .flex_direction(FlexDirection::Column)
                                .gap(1.0)
                                .width(40)
                                .child(
                                    // Input widget
                                    BoxNode::new()
                                        .flex_direction(FlexDirection::Column)
                                        .child(
                                            TextNode::new(
                                                if state.focused == FocusedWidget::Input {
                                                    "Input [focused]"
                                                } else {
                                                    "Input"
                                                },
                                            )
                                            .color(if state.focused == FocusedWidget::Input {
                                                Color::BrightYellow
                                            } else {
                                                Color::White
                                            })
                                            .bold(),
                                        )
                                        .child(
                                            state
                                                .input
                                                .clone()
                                                .focused(state.focused == FocusedWidget::Input),
                                        ),
                                )
                                .child(
                                    // Select widget
                                    BoxNode::new()
                                        .flex_direction(FlexDirection::Column)
                                        .child(
                                            TextNode::new(
                                                if state.focused == FocusedWidget::Select {
                                                    "Select [focused]"
                                                } else {
                                                    "Select"
                                                },
                                            )
                                            .color(if state.focused == FocusedWidget::Select {
                                                Color::BrightYellow
                                            } else {
                                                Color::White
                                            })
                                            .bold(),
                                        )
                                        .child(
                                            Select::new()
                                                .options(select_options.clone())
                                                .selected(state.select_index)
                                                .focused(state.focused == FocusedWidget::Select)
                                                .border(BorderStyle::Single),
                                        ),
                                )
                                .child(
                                    // Progress widget
                                    BoxNode::new()
                                        .flex_direction(FlexDirection::Column)
                                        .child(
                                            TextNode::new(
                                                if state.focused == FocusedWidget::Progress {
                                                    format!(
                                                        "Progress [focused] - {:.0}%",
                                                        state.progress * 100.0
                                                    )
                                                } else {
                                                    format!(
                                                        "Progress - {:.0}%",
                                                        state.progress * 100.0
                                                    )
                                                },
                                            )
                                            .color(if state.focused == FocusedWidget::Progress {
                                                Color::BrightYellow
                                            } else {
                                                Color::White
                                            })
                                            .bold(),
                                        )
                                        .child(
                                            Progress::new()
                                                .progress(state.progress)
                                                .width(30)
                                                .style(ProgressStyle::Block),
                                        )
                                        .child(
                                            Progress::new()
                                                .progress(state.progress)
                                                .width(30)
                                                .style(ProgressStyle::Ascii),
                                        ),
                                ),
                        )
                        .child(
                            // Right column
                            BoxNode::new()
                                .flex_direction(FlexDirection::Column)
                                .gap(1.0)
                                .flex_grow(1.0)
                                .child(
                                    // Scroll widget
                                    BoxNode::new()
                                        .flex_direction(FlexDirection::Column)
                                        .child(
                                            TextNode::new(
                                                if state.focused == FocusedWidget::Scroll {
                                                    format!(
                                                        "Scroll [focused] - offset: {}",
                                                        state.scroll_offset
                                                    )
                                                } else {
                                                    format!(
                                                        "Scroll - offset: {}",
                                                        state.scroll_offset
                                                    )
                                                },
                                            )
                                            .color(if state.focused == FocusedWidget::Scroll {
                                                Color::BrightYellow
                                            } else {
                                                Color::White
                                            })
                                            .bold(),
                                        )
                                        .child(
                                            Scroll::new()
                                                .height(8)
                                                .offset_y(state.scroll_offset)
                                                .content_height(scroll_lines.len() as u16)
                                                .scrollbar(ScrollbarVisibility::Auto)
                                                .border(BorderStyle::Single)
                                                .children(
                                                    scroll_lines
                                                        .iter()
                                                        .map(|line| TextNode::new(line.clone())),
                                                ),
                                        ),
                                )
                                .child(
                                    // Spinner showcase
                                    BoxNode::new()
                                        .flex_direction(FlexDirection::Column)
                                        .child(TextNode::new("Spinners").color(Color::White).bold())
                                        .child(
                                            BoxNode::new()
                                                .flex_direction(FlexDirection::Row)
                                                .gap(2.0)
                                                .child(
                                                    BoxNode::new()
                                                        .flex_direction(FlexDirection::Row)
                                                        .gap(1.0)
                                                        .child(
                                                            Spinner::new()
                                                                .frame(state.spinner_frame),
                                                        )
                                                        .child(TextNode::new("Dots")),
                                                )
                                                .child(
                                                    BoxNode::new()
                                                        .flex_direction(FlexDirection::Row)
                                                        .gap(1.0)
                                                        .child(
                                                            Spinner::new()
                                                                .style(SpinnerStyle::Line)
                                                                .frame(state.spinner_frame),
                                                        )
                                                        .child(TextNode::new("Line")),
                                                )
                                                .child(
                                                    BoxNode::new()
                                                        .flex_direction(FlexDirection::Row)
                                                        .gap(1.0)
                                                        .child(
                                                            Spinner::new()
                                                                .style(SpinnerStyle::Arrow)
                                                                .frame(state.spinner_frame),
                                                        )
                                                        .child(TextNode::new("Arrow")),
                                                ),
                                        ),
                                )
                                .child(
                                    // Stack demo
                                    BoxNode::new()
                                        .flex_direction(FlexDirection::Column)
                                        .child(
                                            TextNode::new("Stack (layered content)")
                                                .color(Color::White)
                                                .bold(),
                                        )
                                        .child(
                                            Stack::new()
                                                .layer(
                                                    TextNode::new("Base layer")
                                                        .color(Color::BrightBlack),
                                                )
                                                .layer(
                                                    TextNode::new("Top layer")
                                                        .color(Color::BrightGreen),
                                                ),
                                        ),
                                ),
                        ),
                )
                .child(Spacer::new())
                .child(
                    TextNode::new("Built with inky - The Terminal UI Framework for the AI Era")
                        .color(Color::BrightBlack)
                        .italic(),
                )
                .into()
        })
        .on_key(|state, key| {
            match key.code {
                // Quit
                KeyCode::Char('q') | KeyCode::Esc => return true,

                // Switch focus
                KeyCode::Tab => {
                    if key.modifiers.shift {
                        state.focused = state.focused.prev();
                    } else {
                        state.focused = state.focused.next();
                    }
                }

                // Handle input based on focused widget
                KeyCode::Up => match state.focused {
                    FocusedWidget::Select => {
                        if state.select_index > 0 {
                            state.select_index -= 1;
                        }
                    }
                    FocusedWidget::Progress => {
                        state.progress = (state.progress + 0.1).min(1.0);
                    }
                    FocusedWidget::Scroll => {
                        if state.scroll_offset > 0 {
                            state.scroll_offset -= 1;
                        }
                    }
                    _ => {}
                },
                KeyCode::Down => match state.focused {
                    FocusedWidget::Select => {
                        if state.select_index < 4 {
                            state.select_index += 1;
                        }
                    }
                    FocusedWidget::Progress => {
                        state.progress = (state.progress - 0.1).max(0.0);
                    }
                    FocusedWidget::Scroll => {
                        if state.scroll_offset < 22 {
                            // 30 lines - 8 viewport
                            state.scroll_offset += 1;
                        }
                    }
                    _ => {}
                },
                KeyCode::Left => {
                    if state.focused == FocusedWidget::Input {
                        state.input.move_left();
                    }
                }
                KeyCode::Right => {
                    if state.focused == FocusedWidget::Input {
                        state.input.move_right();
                    }
                }
                KeyCode::Backspace => {
                    if state.focused == FocusedWidget::Input {
                        state.input.backspace();
                    }
                }
                KeyCode::Delete => {
                    if state.focused == FocusedWidget::Input {
                        state.input.delete();
                    }
                }
                KeyCode::Home => {
                    if state.focused == FocusedWidget::Input {
                        state.input.move_home();
                    } else if state.focused == FocusedWidget::Scroll {
                        state.scroll_offset = 0;
                    }
                }
                KeyCode::End => {
                    if state.focused == FocusedWidget::Input {
                        state.input.move_end();
                    } else if state.focused == FocusedWidget::Scroll {
                        state.scroll_offset = 22;
                    }
                }
                KeyCode::PageUp => {
                    if state.focused == FocusedWidget::Scroll {
                        state.scroll_offset = state.scroll_offset.saturating_sub(8);
                    }
                }
                KeyCode::PageDown => {
                    if state.focused == FocusedWidget::Scroll {
                        state.scroll_offset = (state.scroll_offset + 8).min(22);
                    }
                }
                KeyCode::Char(c) => {
                    if state.focused == FocusedWidget::Input {
                        state.input.insert(c);
                    }
                    // Animate spinners on any key
                    state.spinner_frame = (state.spinner_frame + 1) % 10;
                }
                _ => {
                    // Animate spinners
                    state.spinner_frame = (state.spinner_frame + 1) % 10;
                }
            }
            false
        })
        .run()?;

    Ok(())
}
