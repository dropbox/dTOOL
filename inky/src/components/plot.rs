//! Plot component - line and bar charts.
//!
//! Renders data series as multi-row charts using Unicode characters.
//! Supports line charts, bar charts, and scatter plots.

use crate::components::adaptive::{AdaptiveComponent, Tier0Fallback, TierFeatures};
use crate::node::{BoxNode, Node, TextNode};
use crate::style::{Color, FlexDirection};
use crate::terminal::RenderTier;

/// Plot chart type.
#[derive(Debug, Clone, Copy, Default)]
pub enum PlotType {
    /// Line chart connecting points
    #[default]
    Line,
    /// Bar chart (vertical bars)
    Bar,
    /// Scatter plot (dots)
    Scatter,
    /// Area chart (filled under line)
    Area,
}

/// Data series for plotting.
#[derive(Debug, Clone)]
pub struct Series {
    /// Series name.
    pub name: String,
    /// Data values.
    pub data: Vec<f32>,
    /// Series color.
    pub color: Color,
    /// Marker character (for scatter/line).
    pub marker: char,
}

impl Series {
    /// Create a new series.
    pub fn new(name: impl Into<String>, data: Vec<f32>) -> Self {
        Self {
            name: name.into(),
            data,
            color: Color::White,
            marker: '●',
        }
    }

    /// Set the series color.
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Set the marker character.
    pub fn marker(mut self, marker: char) -> Self {
        self.marker = marker;
        self
    }
}

/// Internal flat grid for cache-efficient plot rendering.
struct PlotGrid {
    cells: Vec<(char, Option<Color>)>,
    width: usize,
    height: usize,
}

impl PlotGrid {
    fn new(width: usize, height: usize) -> Self {
        Self {
            cells: vec![(' ', None); width * height],
            width,
            height,
        }
    }

    #[inline]
    fn get_mut(&mut self, row: usize, col: usize) -> Option<&mut (char, Option<Color>)> {
        if row < self.height && col < self.width {
            Some(&mut self.cells[row * self.width + col])
        } else {
            None
        }
    }

    #[inline]
    fn set(&mut self, row: usize, col: usize, value: (char, Option<Color>)) {
        if row < self.height && col < self.width {
            self.cells[row * self.width + col] = value;
        }
    }

    fn into_nested_vec(self) -> Vec<Vec<(char, Option<Color>)>> {
        self.cells
            .chunks(self.width)
            .map(|row| row.to_vec())
            .collect()
    }
}

/// Multi-row plot component.
///
/// # Example
///
/// ```ignore
/// use inky::prelude::*;
///
/// let plot = Plot::new()
///     .width(60)
///     .height(10)
///     .add_series(Series::new("CPU", vec![10.0, 30.0, 50.0, 40.0, 60.0]))
///     .add_series(Series::new("Mem", vec![20.0, 25.0, 30.0, 35.0, 40.0]).color(Color::Blue))
///     .title("System Stats");
/// ```
#[derive(Debug, Clone)]
pub struct Plot {
    /// Data series.
    series: Vec<Series>,
    /// Chart type.
    plot_type: PlotType,
    /// Chart width in characters.
    width: u16,
    /// Chart height in rows.
    height: u16,
    /// Title.
    title: Option<String>,
    /// X-axis label.
    x_label: Option<String>,
    /// Y-axis label.
    y_label: Option<String>,
    /// Minimum Y value.
    y_min: Option<f32>,
    /// Maximum Y value.
    y_max: Option<f32>,
    /// Show Y-axis labels.
    show_y_axis: bool,
    /// Show X-axis labels.
    show_x_axis: bool,
    /// Show legend.
    show_legend: bool,
    /// Show grid lines.
    show_grid: bool,
    /// X-axis labels.
    x_labels: Option<Vec<String>>,
}

impl Plot {
    /// Create a new empty plot.
    pub fn new() -> Self {
        Self {
            series: Vec::new(),
            plot_type: PlotType::default(),
            width: 60,
            height: 10,
            title: None,
            x_label: None,
            y_label: None,
            y_min: None,
            y_max: None,
            show_y_axis: true,
            show_x_axis: false,
            show_legend: true,
            show_grid: false,
            x_labels: None,
        }
    }

    /// Set the plot type.
    pub fn plot_type(mut self, plot_type: PlotType) -> Self {
        self.plot_type = plot_type;
        self
    }

    /// Set chart width.
    pub fn width(mut self, width: u16) -> Self {
        self.width = width.max(10);
        self
    }

    /// Set chart height.
    pub fn height(mut self, height: u16) -> Self {
        self.height = height.max(3);
        self
    }

    /// Set chart title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set X-axis label.
    pub fn x_label(mut self, label: impl Into<String>) -> Self {
        self.x_label = Some(label.into());
        self
    }

    /// Set Y-axis label.
    pub fn y_label(mut self, label: impl Into<String>) -> Self {
        self.y_label = Some(label.into());
        self
    }

    /// Set Y-axis range.
    pub fn y_range(mut self, min: f32, max: f32) -> Self {
        self.y_min = Some(min);
        self.y_max = Some(max);
        self
    }

    /// Show/hide Y-axis labels.
    pub fn show_y_axis(mut self, show: bool) -> Self {
        self.show_y_axis = show;
        self
    }

    /// Show/hide X-axis labels.
    pub fn show_x_axis(mut self, show: bool) -> Self {
        self.show_x_axis = show;
        self
    }

    /// Show/hide legend.
    pub fn show_legend(mut self, show: bool) -> Self {
        self.show_legend = show;
        self
    }

    /// Show/hide grid lines.
    pub fn show_grid(mut self, show: bool) -> Self {
        self.show_grid = show;
        self
    }

    /// Set X-axis labels.
    pub fn x_labels(mut self, labels: Vec<String>) -> Self {
        self.x_labels = Some(labels);
        self.show_x_axis = true;
        self
    }

    /// Add a data series.
    pub fn add_series(mut self, series: Series) -> Self {
        self.series.push(series);
        self
    }

    /// Add data as a simple series.
    pub fn data(self, data: Vec<f32>) -> Self {
        self.add_series(Series::new("data", data))
    }

    /// Get the Y-axis bounds.
    fn y_bounds(&self) -> (f32, f32) {
        let mut min = f32::MAX;
        let mut max = f32::MIN;

        for series in &self.series {
            for &value in &series.data {
                min = min.min(value);
                max = max.max(value);
            }
        }

        let min = self.y_min.unwrap_or(min);
        let max = self.y_max.unwrap_or(max);

        if min >= max {
            (min - 1.0, max + 1.0)
        } else {
            (min, max)
        }
    }

    /// Get the maximum data length.
    fn max_data_len(&self) -> usize {
        self.series.iter().map(|s| s.data.len()).max().unwrap_or(0)
    }

    /// Render the plot to a 2D character grid.
    pub fn render_grid(&self) -> Vec<Vec<(char, Option<Color>)>> {
        let (y_min, y_max) = self.y_bounds();
        let y_range = y_max - y_min;
        let data_len = self.max_data_len();

        let plot_height = self.height as usize;
        let grid_width = self.width as usize;
        let y_axis_width = if self.show_y_axis { 6 } else { 0 };
        let plot_width = grid_width.saturating_sub(y_axis_width);

        // Initialize flat grid for cache-efficient access
        let mut grid = PlotGrid::new(grid_width, plot_height);

        // Draw Y-axis labels
        if self.show_y_axis {
            for row_idx in 0..plot_height {
                let value = y_max - (row_idx as f32 / (plot_height - 1) as f32) * y_range;
                let label = format!("{:>5.1}", value);
                for (col, c) in label.chars().enumerate() {
                    if col < y_axis_width - 1 {
                        grid.set(row_idx, col, (c, None));
                    }
                }
                grid.set(row_idx, y_axis_width - 1, ('│', None));
            }
        }

        // Draw grid lines if enabled
        if self.show_grid {
            let grid_char = '·';
            for row_idx in (0..plot_height).step_by(3) {
                for col in y_axis_width..grid_width {
                    if let Some(cell) = grid.get_mut(row_idx, col) {
                        if cell.0 == ' ' {
                            *cell = (grid_char, Some(Color::BrightBlack));
                        }
                    }
                }
            }
        }

        // Plot each series
        for series in &self.series {
            if series.data.is_empty() {
                continue;
            }

            let x_scale = if data_len > 1 {
                (plot_width as f32 - 1.0) / (data_len as f32 - 1.0)
            } else {
                1.0
            };

            match self.plot_type {
                PlotType::Line | PlotType::Scatter => {
                    for (i, &value) in series.data.iter().enumerate() {
                        let x = y_axis_width + (i as f32 * x_scale).round() as usize;
                        let y_normalized = (value - y_min) / y_range;
                        let row =
                            ((1.0 - y_normalized) * (plot_height - 1) as f32).round() as usize;

                        grid.set(row, x, (series.marker, Some(series.color)));
                    }

                    // Draw connecting lines for line chart
                    if matches!(self.plot_type, PlotType::Line) && series.data.len() > 1 {
                        for i in 0..series.data.len() - 1 {
                            let x1 = y_axis_width + (i as f32 * x_scale).round() as usize;
                            let x2 = y_axis_width + ((i + 1) as f32 * x_scale).round() as usize;

                            let y1_norm = (series.data[i] - y_min) / y_range;
                            let y2_norm = (series.data[i + 1] - y_min) / y_range;

                            let row1 =
                                ((1.0 - y1_norm) * (plot_height - 1) as f32).round() as usize;
                            let row2 =
                                ((1.0 - y2_norm) * (plot_height - 1) as f32).round() as usize;

                            // Draw line between points (simplified)
                            if x2 > x1 {
                                let slope = (row2 as f32 - row1 as f32) / (x2 as f32 - x1 as f32);
                                for x in x1..=x2 {
                                    let row =
                                        (row1 as f32 + slope * (x - x1) as f32).round() as usize;
                                    let line_char = if slope.abs() < 0.5 {
                                        '─'
                                    } else if slope > 0.0 {
                                        '╲'
                                    } else {
                                        '╱'
                                    };
                                    // Don't overwrite markers
                                    if let Some(cell) = grid.get_mut(row, x) {
                                        if cell.0 != series.marker {
                                            *cell = (line_char, Some(series.color));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                PlotType::Bar => {
                    let bar_width = ((plot_width as f32) / (data_len as f32)).floor() as usize;
                    let bar_width = bar_width.max(1);

                    for (i, &value) in series.data.iter().enumerate() {
                        let x_start = y_axis_width + i * bar_width;
                        let y_normalized = (value - y_min) / y_range;
                        let bar_height = (y_normalized * plot_height as f32).round() as usize;

                        for row in (plot_height - bar_height)..plot_height {
                            for x in
                                x_start..(x_start + bar_width.saturating_sub(1)).min(grid_width)
                            {
                                grid.set(row, x, ('█', Some(series.color)));
                            }
                        }
                    }
                }
                PlotType::Area => {
                    for (i, &value) in series.data.iter().enumerate() {
                        let x = y_axis_width + (i as f32 * x_scale).round() as usize;
                        let y_normalized = (value - y_min) / y_range;
                        let top_row =
                            ((1.0 - y_normalized) * (plot_height - 1) as f32).round() as usize;

                        for row in top_row..plot_height {
                            if let Some(cell) = grid.get_mut(row, x) {
                                if cell.0 == ' ' || cell.0 == '·' {
                                    let fill_char = if row == top_row { '▀' } else { '░' };
                                    *cell = (fill_char, Some(series.color));
                                }
                            }
                        }
                    }
                }
            }
        }

        grid.into_nested_vec()
    }
}

impl Default for Plot {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Plot> for Node {
    fn from(plot: Plot) -> Self {
        let mut container = BoxNode::new().flex_direction(FlexDirection::Column);

        // Add title if present
        if let Some(title) = &plot.title {
            let title_node = TextNode::new(title).bold();
            container = container.child(
                BoxNode::new()
                    .flex_direction(FlexDirection::Row)
                    .justify_content(crate::style::JustifyContent::Center)
                    .child(title_node),
            );
        }

        // Render the plot grid
        let grid = plot.render_grid();

        for row in grid {
            let mut row_box = BoxNode::new().flex_direction(FlexDirection::Row);
            let mut current_text = String::new();
            let mut current_color: Option<Color> = None;

            for (ch, color) in row {
                if color == current_color {
                    current_text.push(ch);
                } else {
                    if !current_text.is_empty() {
                        let mut text_node = TextNode::new(&current_text);
                        if let Some(c) = current_color {
                            text_node = text_node.color(c);
                        }
                        row_box = row_box.child(text_node);
                        current_text.clear();
                    }
                    current_text.push(ch);
                    current_color = color;
                }
            }

            // Add remaining text
            if !current_text.is_empty() {
                let mut text_node = TextNode::new(current_text);
                if let Some(c) = current_color {
                    text_node = text_node.color(c);
                }
                row_box = row_box.child(text_node);
            }

            container = container.child(row_box);
        }

        // Add X-axis label if present
        if let Some(x_label) = &plot.x_label {
            container = container.child(
                BoxNode::new()
                    .flex_direction(FlexDirection::Row)
                    .justify_content(crate::style::JustifyContent::Center)
                    .child(TextNode::new(x_label)),
            );
        }

        // Add legend if enabled and there are multiple series
        if plot.show_legend && plot.series.len() > 1 {
            let mut legend_box = BoxNode::new().flex_direction(FlexDirection::Row).gap(2.0);

            for series in &plot.series {
                let marker = TextNode::new(format!("{} ", series.marker)).color(series.color);
                let name = TextNode::new(&series.name);
                legend_box = legend_box.child(
                    BoxNode::new()
                        .flex_direction(FlexDirection::Row)
                        .child(marker)
                        .child(name),
                );
            }

            container = container.child(legend_box);
        }

        container.into()
    }
}

impl AdaptiveComponent for Plot {
    fn render_for_tier(&self, tier: RenderTier) -> Node {
        match tier {
            RenderTier::Tier0Fallback => self.render_tier0(),
            RenderTier::Tier1Ansi => self.render_tier1(),
            RenderTier::Tier2Retained | RenderTier::Tier3Gpu => self.clone().into(),
        }
    }

    fn tier_features(&self) -> TierFeatures {
        TierFeatures::new("Plot")
            .tier0("Text summary with statistics")
            .tier1("ASCII chart representation")
            .tier2("Unicode chart with colors")
            .tier3("GPU-accelerated rendering")
    }

    fn minimum_tier(&self) -> Option<RenderTier> {
        None // Works at all tiers
    }
}

impl Plot {
    /// Render Tier 0: Text-only statistics summary.
    fn render_tier0(&self) -> Node {
        let (y_min, y_max) = self.y_bounds();
        let data_len = self.max_data_len();

        // Calculate mean across all series
        let mean = if data_len > 0 {
            let total: f32 = self.series.iter().flat_map(|s| s.data.iter()).sum();
            let count = self.series.iter().map(|s| s.data.len()).sum::<usize>();
            if count > 0 {
                total / count as f32
            } else {
                0.0
            }
        } else {
            0.0
        };

        let label = self.title.as_deref().unwrap_or("Plot");

        Tier0Fallback::new(label)
            .stat("series", self.series.len().to_string())
            .stat("points", data_len.to_string())
            .stat("min", format!("{:.1}", y_min))
            .stat("max", format!("{:.1}", y_max))
            .stat("mean", format!("{:.1}", mean))
            .into()
    }

    /// Render Tier 1: Simple ASCII chart.
    fn render_tier1(&self) -> Node {
        // Use simple ASCII characters for chart
        const ASCII_CHARS: &[char] = &[' ', '_', '.', '-', '=', '#'];

        let (y_min, y_max) = self.y_bounds();
        let y_range = if (y_max - y_min).abs() < f32::EPSILON {
            1.0
        } else {
            y_max - y_min
        };

        let width = self.width.min(60) as usize;
        let height = self.height.min(10) as usize;

        let mut rows: Vec<String> = Vec::with_capacity(height);

        // Build ASCII grid
        for row in 0..height {
            let mut line = String::with_capacity(width);
            let row_value = y_max - (row as f32 / (height - 1).max(1) as f32) * y_range;

            for col in 0..width {
                let x_idx = if width > 1 {
                    (col * self.max_data_len()) / width
                } else {
                    0
                };

                // Get value from first series at this position
                let value = self
                    .series
                    .first()
                    .and_then(|s| s.data.get(x_idx))
                    .copied()
                    .unwrap_or(y_min);

                // Determine character based on value relative to row
                let diff = (value - row_value).abs() / y_range;
                let char_idx = if diff < 0.1 {
                    5 // '#' - on the line
                } else if diff < 0.2 {
                    4 // '='
                } else if diff < 0.3 {
                    3 // '-'
                } else if diff < 0.4 {
                    2 // '.'
                } else {
                    // 1 = below line, 0 = above line
                    usize::from(value < row_value)
                };

                line.push(ASCII_CHARS[char_idx]);
            }

            rows.push(line);
        }

        // Build node
        let mut container = BoxNode::new().flex_direction(FlexDirection::Column);

        // Add title if present
        if let Some(title) = &self.title {
            container = container.child(TextNode::new(title));
        }

        // Add rows
        for row in rows {
            container = container.child(TextNode::new(row));
        }

        // Add simple legend
        if self.show_legend && !self.series.is_empty() {
            let legend: Vec<String> = self
                .series
                .iter()
                .map(|s| format!("[{}]", s.name))
                .collect();
            container = container.child(TextNode::new(legend.join(" ")));
        }

        container.into()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_plot_new() {
        let plot = Plot::new();
        assert_eq!(plot.width, 60);
        assert_eq!(plot.height, 10);
    }

    #[test]
    fn test_plot_with_data() {
        let plot = Plot::new().data(vec![1.0, 2.0, 3.0, 2.0, 1.0]);

        assert_eq!(plot.max_data_len(), 5);
    }

    #[test]
    fn test_plot_y_bounds() {
        let plot = Plot::new().add_series(Series::new("test", vec![10.0, 20.0, 30.0]));

        let (min, max) = plot.y_bounds();
        assert_eq!(min, 10.0);
        assert_eq!(max, 30.0);
    }

    #[test]
    fn test_plot_custom_y_range() {
        let plot = Plot::new().data(vec![10.0, 20.0, 30.0]).y_range(0.0, 100.0);

        let (min, max) = plot.y_bounds();
        assert_eq!(min, 0.0);
        assert_eq!(max, 100.0);
    }

    #[test]
    fn test_plot_render_grid() {
        let plot = Plot::new()
            .width(20)
            .height(5)
            .show_y_axis(false)
            .data(vec![0.0, 0.5, 1.0, 0.5, 0.0]);

        let grid = plot.render_grid();
        assert_eq!(grid.len(), 5);
        assert_eq!(grid[0].len(), 20);
    }

    #[test]
    fn test_series_builder() {
        let series = Series::new("CPU", vec![1.0, 2.0, 3.0])
            .color(Color::Green)
            .marker('*');

        assert_eq!(series.name, "CPU");
        assert_eq!(series.color, Color::Green);
        assert_eq!(series.marker, '*');
    }

    #[test]
    fn test_plot_multiple_series() {
        let plot = Plot::new()
            .add_series(Series::new("A", vec![1.0, 2.0]).color(Color::Red))
            .add_series(Series::new("B", vec![2.0, 1.0]).color(Color::Blue));

        assert_eq!(plot.series.len(), 2);
    }

    #[test]
    fn test_plot_to_node() {
        let plot = Plot::new()
            .width(30)
            .height(8)
            .title("Test Plot")
            .data(vec![1.0, 2.0, 3.0]);

        let node: Node = plot.into();
        assert!(matches!(node, Node::Box(_)));
    }

    #[test]
    fn test_plot_types() {
        let data = vec![1.0, 3.0, 2.0, 4.0, 3.0];

        for plot_type in [
            PlotType::Line,
            PlotType::Bar,
            PlotType::Scatter,
            PlotType::Area,
        ] {
            let plot = Plot::new()
                .width(20)
                .height(5)
                .plot_type(plot_type)
                .data(data.clone());

            let grid = plot.render_grid();
            assert_eq!(grid.len(), 5);
        }
    }

    #[test]
    fn test_plot_empty() {
        let plot = Plot::new();
        let grid = plot.render_grid();
        assert_eq!(grid.len(), 10);
    }

    #[test]
    fn test_adaptive_tier0() {
        let plot = Plot::new().title("Test").data(vec![1.0, 2.0, 3.0]);

        let node = plot.render_for_tier(RenderTier::Tier0Fallback);
        assert!(matches!(node, Node::Text(_)));
    }

    #[test]
    fn test_adaptive_tier1() {
        let plot = Plot::new()
            .width(20)
            .height(5)
            .data(vec![1.0, 3.0, 2.0, 4.0, 3.0]);

        let node = plot.render_for_tier(RenderTier::Tier1Ansi);
        assert!(matches!(node, Node::Box(_)));
    }

    #[test]
    fn test_adaptive_tier2() {
        let plot = Plot::new().data(vec![1.0, 2.0, 3.0]);

        let node = plot.render_for_tier(RenderTier::Tier2Retained);
        assert!(matches!(node, Node::Box(_)));
    }

    #[test]
    fn test_adaptive_all_tiers() {
        let plot = Plot::new()
            .title("System Metrics")
            .add_series(Series::new("CPU", vec![10.0, 30.0, 50.0]).color(Color::Green))
            .add_series(Series::new("MEM", vec![20.0, 25.0, 30.0]).color(Color::Blue));

        // Should render without panic at all tiers
        for tier in [
            RenderTier::Tier0Fallback,
            RenderTier::Tier1Ansi,
            RenderTier::Tier2Retained,
            RenderTier::Tier3Gpu,
        ] {
            let _ = plot.render_for_tier(tier);
        }
    }

    #[test]
    fn test_plot_tier_features() {
        let plot = Plot::new();
        let features = plot.tier_features();

        assert_eq!(features.name, Some("Plot"));
        assert!(features.tier0_description.is_some());
        assert!(features.tier1_description.is_some());
        assert!(features.tier2_description.is_some());
        assert!(features.tier3_description.is_some());
    }
}
