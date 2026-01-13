//! Multi-pane dashboard example demonstrating complex layouts.
//!
//! Shows a realistic dashboard with:
//! - Header with title
//! - Sidebar with navigation menu
//! - Main content area with multiple panels
//! - Status bar at the bottom
//!
//! Controls:
//! - Tab: Cycle through panels
//! - Arrow keys: Navigate within focused panel
//! - q / Escape: Quit

use inky::prelude::*;
use std::time::Duration;

/// Dashboard state
struct DashboardState {
    /// Currently selected sidebar menu item
    selected_menu: Signal<usize>,
    /// Timer for animations
    timer: IntervalHandle,
    /// Menu items
    menu_items: Vec<&'static str>,
}

impl DashboardState {
    fn new() -> Self {
        Self {
            selected_menu: use_signal(0),
            timer: use_interval(Duration::from_millis(500)),
            menu_items: vec!["Overview", "Analytics", "Reports", "Settings"],
        }
    }

    fn menu_next(&self) {
        let max = self.menu_items.len() - 1;
        self.selected_menu.update(|i| {
            if *i < max {
                *i += 1;
            }
        });
    }

    fn menu_prev(&self) {
        self.selected_menu.update(|i| {
            if *i > 0 {
                *i -= 1;
            }
        });
    }
}

/// Build the header section
fn build_header(title: &str, width: u16) -> BoxNode {
    BoxNode::new()
        .width(width)
        .height(3)
        .border(BorderStyle::Single)
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .flex_direction(FlexDirection::Row)
        .child(TextNode::new(title).bold().color(Color::BrightCyan))
}

/// Build the sidebar menu
fn build_sidebar(items: &[&str], selected: usize, height: u16) -> BoxNode {
    let mut sidebar = BoxNode::new()
        .width(20)
        .height(height)
        .border(BorderStyle::Single)
        .flex_direction(FlexDirection::Column)
        .padding(1);

    sidebar = sidebar.child(TextNode::new("Navigation").bold().color(Color::Yellow));
    sidebar = sidebar.child(TextNode::new(""));

    for (i, item) in items.iter().enumerate() {
        let text = if i == selected {
            format!("> {}", item)
        } else {
            format!("  {}", item)
        };

        let color = if i == selected {
            Color::BrightGreen
        } else {
            Color::White
        };

        sidebar = sidebar.child(TextNode::new(text).color(color));
    }

    sidebar
}

/// Build a stat card
fn build_stat_card(title: &str, value: &str, trend: &str, color: Color) -> BoxNode {
    BoxNode::new()
        .flex_grow(1.0)
        .height(5)
        .border(BorderStyle::Rounded)
        .flex_direction(FlexDirection::Column)
        .padding(1)
        .child(TextNode::new(title).color(Color::BrightBlack))
        .child(TextNode::new(value).bold().color(color))
        .child(TextNode::new(trend).color(Color::BrightBlack))
}

/// Build the stats row
fn build_stats_row(tick: u64) -> BoxNode {
    let users = 1234 + (tick % 100) as i64;
    let revenue = format!("${:.2}K", 42.5 + (tick % 10) as f64 * 0.1);
    let orders = 89 + (tick % 20);
    let conversion = format!("{:.1}%", 3.2 + (tick % 5) as f32 * 0.1);

    BoxNode::new()
        .flex_direction(FlexDirection::Row)
        .gap(2.0)
        .child(build_stat_card(
            "Active Users",
            &users.to_string(),
            "+12%",
            Color::BrightGreen,
        ))
        .child(build_stat_card(
            "Revenue",
            &revenue,
            "+8.5%",
            Color::BrightBlue,
        ))
        .child(build_stat_card(
            "Orders",
            &orders.to_string(),
            "+23",
            Color::BrightYellow,
        ))
        .child(build_stat_card(
            "Conversion",
            &conversion,
            "+0.3%",
            Color::BrightMagenta,
        ))
}

/// Build a simple bar chart
fn build_bar_chart(tick: u64) -> BoxNode {
    let mut chart = BoxNode::new()
        .flex_grow(1.0)
        .border(BorderStyle::Single)
        .flex_direction(FlexDirection::Column)
        .padding(1);

    chart = chart.child(TextNode::new("Weekly Traffic").bold());
    chart = chart.child(TextNode::new(""));

    let days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let values = [45, 62, 58, 71, 65, 38, 42];

    for (i, (day, &base_val)) in days.iter().zip(values.iter()).enumerate() {
        let val = base_val + ((tick + i as u64) % 10) as usize;
        let bar_len = val / 5;
        let bar: String = "█".repeat(bar_len);
        chart = chart
            .child(TextNode::new(format!("{}: {:>3} {}", day, val, bar)).color(Color::BrightCyan));
    }

    chart
}

/// Build activity log
fn build_activity_log(tick: u64) -> BoxNode {
    let activities = [
        "User signed up",
        "Order completed",
        "Payment received",
        "Item shipped",
        "Review posted",
    ];

    let mut log = BoxNode::new()
        .flex_grow(1.0)
        .border(BorderStyle::Single)
        .flex_direction(FlexDirection::Column)
        .padding(1);

    log = log.child(TextNode::new("Recent Activity").bold());
    log = log.child(TextNode::new(""));

    for i in 0..5 {
        let idx = (tick as usize + i) % activities.len();
        let time = format!("{}m ago", (i * 3) + (tick as usize % 5));
        log = log.child(
            TextNode::new(format!("[{}] {}", time, activities[idx])).color(if i == 0 {
                Color::BrightGreen
            } else {
                Color::White
            }),
        );
    }

    log
}

/// Build status bar
fn build_status_bar(width: u16, tick: u64) -> BoxNode {
    let status = if tick % 4 < 2 { "●" } else { "○" };
    let time = format!(
        "{:02}:{:02}:{:02}",
        tick / 3600,
        (tick / 60) % 60,
        tick % 60
    );

    BoxNode::new()
        .width(width)
        .height(1)
        .flex_direction(FlexDirection::Row)
        .justify_content(JustifyContent::SpaceBetween)
        .child(TextNode::new(format!("{} Connected", status)).color(Color::BrightGreen))
        .child(TextNode::new("Tab: navigate | q: quit").color(Color::BrightBlack))
        .child(TextNode::new(time).color(Color::BrightBlack))
}

fn main() -> Result<()> {
    let state = DashboardState::new();

    App::new()
        .state(state)
        .alt_screen(true)
        .render(|ctx| {
            let tick = ctx.state.timer.get();
            let selected = ctx.state.selected_menu.get();
            let width = ctx.width();
            let height = ctx.height();

            let content_height = height.saturating_sub(5);
            let _main_width = width.saturating_sub(22);

            // Build the main content based on selected menu
            let content_title = ctx.state.menu_items[selected];
            let main_content = BoxNode::new()
                .flex_grow(1.0)
                .height(content_height)
                .flex_direction(FlexDirection::Column)
                .gap(1.0)
                .child(TextNode::new(format!("{} Dashboard", content_title)).bold())
                .child(build_stats_row(tick))
                .child(
                    BoxNode::new()
                        .flex_direction(FlexDirection::Row)
                        .gap(2.0)
                        .flex_grow(1.0)
                        .child(build_bar_chart(tick))
                        .child(build_activity_log(tick)),
                );

            // Assemble the full dashboard
            BoxNode::new()
                .width(width)
                .height(height)
                .flex_direction(FlexDirection::Column)
                .child(build_header("inky Dashboard", width))
                .child(
                    BoxNode::new()
                        .flex_direction(FlexDirection::Row)
                        .flex_grow(1.0)
                        .child(build_sidebar(
                            &ctx.state.menu_items,
                            selected,
                            content_height,
                        ))
                        .child(main_content),
                )
                .child(build_status_bar(width, tick))
                .into()
        })
        .on_key(|state, key| {
            match key.code {
                KeyCode::Tab | KeyCode::Down | KeyCode::Char('j') => {
                    state.menu_next();
                }
                KeyCode::BackTab | KeyCode::Up | KeyCode::Char('k') => {
                    state.menu_prev();
                }
                KeyCode::Char('q') | KeyCode::Esc => {
                    return true;
                }
                _ => {}
            }
            false
        })
        .run()?;

    Ok(())
}
