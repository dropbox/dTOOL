//! Visualization example with heatmap, sparklines, and plot.
//!
//! Demonstrates GPU-friendly components with live updating data.

use inky::prelude::*;
use std::time::Duration;

struct VizState {
    timer: IntervalHandle,
}

impl VizState {
    fn new() -> Self {
        Self {
            timer: use_interval(Duration::from_millis(150)),
        }
    }
}

fn build_heatmap(rows: usize, cols: usize, phase: f32) -> Vec<Vec<f32>> {
    let mut data = Vec::with_capacity(rows);
    let row_scale = std::f32::consts::TAU / rows as f32;
    let col_scale = std::f32::consts::TAU / cols as f32;

    for row in 0..rows {
        let mut row_data = Vec::with_capacity(cols);
        let y = row as f32 * row_scale;
        for col in 0..cols {
            let x = col as f32 * col_scale;
            let value = (x + phase).sin() * 0.5 + 0.5;
            let ripple = (y - phase * 0.7).cos() * 0.5 + 0.5;
            row_data.push(value * ripple);
        }
        data.push(row_data);
    }

    data
}

fn build_series(len: usize, phase: f32, freq: f32, base: f32) -> Vec<f32> {
    (0..len)
        .map(|i| {
            let t = phase + i as f32 * 0.2;
            let wave = (t * freq).sin() * 0.5 + 0.5;
            base + wave * 40.0
        })
        .collect()
}

fn main() -> Result<()> {
    let state = VizState::new();

    App::new()
        .state(state)
        .alt_screen(true)
        .render(|ctx| {
            let tick = ctx.state.timer.get() as f32;
            let phase = tick * 0.15;

            let heatmap_data = build_heatmap(10, 16, phase);
            let cpu_series = build_series(32, phase, 1.1, 40.0);
            let mem_series = build_series(32, phase + 1.3, 0.8, 30.0);
            let io_series = build_series(32, phase + 2.6, 1.4, 20.0);

            let plot_cpu = build_series(48, phase * 0.8, 0.7, 25.0);
            let plot_mem = build_series(48, phase * 0.8 + 1.0, 0.7, 20.0);

            let heatmap = Heatmap::new(heatmap_data)
                .palette(HeatmapPalette::Viridis)
                .style(HeatmapStyle::Background)
                .cell_width(2);

            let cpu_spark = Sparkline::new(cpu_series)
                .label("CPU")
                .color(Color::BrightGreen)
                .show_value(true)
                .show_range(true)
                .max_width(32);

            let mem_spark = Sparkline::new(mem_series)
                .label("MEM")
                .color(Color::BrightBlue)
                .show_value(true)
                .show_range(true)
                .max_width(32);

            let io_spark = Sparkline::new(io_series)
                .label("IO")
                .color(Color::BrightYellow)
                .show_value(true)
                .show_range(true)
                .max_width(32)
                .style(SparklineStyle::Line);

            let plot = Plot::new()
                .width(64)
                .height(10)
                .plot_type(PlotType::Line)
                .title("System Trends")
                .show_grid(true)
                .add_series(Series::new("CPU", plot_cpu).color(Color::BrightGreen))
                .add_series(Series::new("MEM", plot_mem).color(Color::BrightBlue));

            BoxNode::new()
                .width(ctx.width())
                .height(ctx.height())
                .flex_direction(FlexDirection::Column)
                .padding(1)
                .gap(1.0)
                .child(
                    TextNode::new("inky Visualization Dashboard")
                        .bold()
                        .color(Color::BrightCyan),
                )
                .child(
                    BoxNode::new()
                        .flex_direction(FlexDirection::Row)
                        .gap(4.0)
                        .child(heatmap)
                        .child(
                            BoxNode::new()
                                .flex_direction(FlexDirection::Column)
                                .gap(1.0)
                                .child(TextNode::new("Live Metrics").bold())
                                .child(cpu_spark)
                                .child(mem_spark)
                                .child(io_spark),
                        ),
                )
                .child(Spacer::new())
                .child(plot)
                .child(TextNode::new("Press q or Esc to quit").color(Color::BrightBlack))
                .into()
        })
        .on_key(|_, key| matches!(key.code, KeyCode::Char('q') | KeyCode::Esc))
        .run()?;

    Ok(())
}
