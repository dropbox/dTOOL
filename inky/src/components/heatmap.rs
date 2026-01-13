//! Heatmap component - 2D data visualization.
//!
//! Renders a grid of values as colored cells using Unicode block characters
//! or terminal background colors. Optimized for GPU rendering when available.
//!
//! # Adaptive Rendering
//!
//! `Heatmap` implements [`AdaptiveComponent`] for graceful degradation:
//!
//! | Tier | Rendering |
//! |------|-----------|
//! | 0 (Fallback) | Text summary: min, max, mean, dimensions |
//! | 1 (ANSI) | ASCII density characters (` .:;oO@#`) |
//! | 2 (Retained) | Unicode blocks with true color |
//! | 3 (GPU) | GPU-accelerated rendering (same as Tier 2 for now) |

use crate::components::adaptive::{AdaptiveComponent, Tier0Fallback, TierFeatures};
use crate::node::{BoxNode, Node, TextNode};
use crate::style::{Color, FlexDirection};
use crate::terminal::RenderTier;

/// Color palette for heatmap rendering.
#[derive(Debug, Clone, Copy, Default)]
pub enum HeatmapPalette {
    /// Grayscale: black to white
    #[default]
    Grayscale,
    /// Heat: black → red → yellow → white
    Heat,
    /// Cool: black → blue → cyan → white
    Cool,
    /// Viridis: scientific visualization palette.
    ///
    /// This palette is colorblind-safe and perceptually uniform, making it
    /// an excellent choice for accessible data visualization.
    Viridis,
    /// Plasma: perceptually uniform
    Plasma,
    /// Red to green (for diff visualization).
    ///
    /// **Warning:** This palette is not colorblind-safe. Approximately 8% of men
    /// and 0.5% of women have red-green color blindness. Consider using
    /// [`Viridis`](Self::Viridis) or [`Grayscale`](Self::Grayscale) for
    /// accessible visualizations.
    RedGreen,
}

impl HeatmapPalette {
    /// Convert a normalized value (0.0-1.0) to an RGB color.
    pub fn to_rgb(&self, value: f32) -> (u8, u8, u8) {
        let v = value.clamp(0.0, 1.0);

        match self {
            HeatmapPalette::Grayscale => {
                let g = (v * 255.0) as u8;
                (g, g, g)
            }
            HeatmapPalette::Heat => {
                // Black → Red → Yellow → White
                if v < 0.33 {
                    let t = v / 0.33;
                    ((t * 255.0) as u8, 0, 0)
                } else if v < 0.66 {
                    let t = (v - 0.33) / 0.33;
                    (255, (t * 255.0) as u8, 0)
                } else {
                    let t = ((v - 0.66) / 0.34).min(1.0);
                    (255, 255, (t * 255.0).round() as u8)
                }
            }
            HeatmapPalette::Cool => {
                // Black → Blue → Cyan → White
                if v < 0.33 {
                    let t = v / 0.33;
                    (0, 0, (t * 255.0) as u8)
                } else if v < 0.66 {
                    let t = (v - 0.33) / 0.33;
                    (0, (t * 255.0) as u8, 255)
                } else {
                    let t = (v - 0.66) / 0.34;
                    ((t * 255.0) as u8, 255, 255)
                }
            }
            HeatmapPalette::Viridis => {
                // Approximation of viridis colormap
                let r = (0.267 + 0.004 * v + v * v * (0.329 - 0.227 * v)) * 255.0;
                let g = (0.004 + v * (1.0 - 0.55 * v * v)) * 255.0;
                let b = (0.329 + 0.42 * v - 0.75 * v * v) * 255.0;
                (
                    r.clamp(0.0, 255.0) as u8,
                    g.clamp(0.0, 255.0) as u8,
                    b.clamp(0.0, 255.0) as u8,
                )
            }
            HeatmapPalette::Plasma => {
                // Approximation of plasma colormap
                let r = (0.05 + 0.93 * v) * 255.0;
                let g = (0.11 * v + 0.4 * v * v) * 255.0;
                let b = (0.53 + 0.47 * (1.0 - v)) * 255.0;
                (
                    r.clamp(0.0, 255.0) as u8,
                    g.clamp(0.0, 255.0) as u8,
                    b.clamp(0.0, 255.0) as u8,
                )
            }
            HeatmapPalette::RedGreen => {
                // Red (low) → White (mid) → Green (high)
                if v < 0.5 {
                    let t = v / 0.5;
                    (255, (t * 255.0) as u8, (t * 255.0) as u8)
                } else {
                    let t = (v - 0.5) / 0.5;
                    ((255.0 * (1.0 - t)) as u8, 255, ((1.0 - t) * 255.0) as u8)
                }
            }
        }
    }

    /// Convert to a Color enum.
    pub fn to_color(&self, value: f32) -> Color {
        let (r, g, b) = self.to_rgb(value);
        Color::Rgb(r, g, b)
    }
}

/// Heatmap rendering style.
#[derive(Debug, Clone, Copy, Default)]
pub enum HeatmapStyle {
    /// Use background colors (best for terminals with true color)
    #[default]
    Background,
    /// Use Unicode block characters with foreground color
    Blocks,
    /// Use half-block characters for 2x vertical resolution
    HalfBlocks,
    /// Use braille patterns for high resolution
    Braille,
}

/// 2D heatmap visualization component.
///
/// # Example
///
/// ```ignore
/// use inky::prelude::*;
///
/// let data = vec![
///     vec![0.0, 0.2, 0.4],
///     vec![0.3, 0.5, 0.7],
///     vec![0.6, 0.8, 1.0],
/// ];
///
/// let heatmap = Heatmap::new(data)
///     .palette(HeatmapPalette::Heat)
///     .cell_width(2);
/// ```
#[derive(Debug, Clone)]
pub struct Heatmap {
    /// 2D data array (row-major, values 0.0-1.0).
    data: Vec<Vec<f32>>,
    /// Color palette.
    palette: HeatmapPalette,
    /// Rendering style.
    style: HeatmapStyle,
    /// Width of each cell in characters.
    cell_width: u16,
    /// Height of each cell in rows (only for Background style).
    cell_height: u16,
    /// Minimum value for normalization.
    min_value: f32,
    /// Maximum value for normalization.
    max_value: f32,
    /// Whether to auto-normalize values.
    auto_normalize: bool,
    /// Show row labels.
    show_row_labels: bool,
    /// Show column labels.
    show_col_labels: bool,
    /// Custom row labels.
    row_labels: Option<Vec<String>>,
    /// Custom column labels.
    col_labels: Option<Vec<String>>,
}

impl Heatmap {
    /// Create a new heatmap with the given 2D data.
    pub fn new(data: Vec<Vec<f32>>) -> Self {
        Self {
            data,
            palette: HeatmapPalette::default(),
            style: HeatmapStyle::default(),
            cell_width: 2,
            cell_height: 1,
            min_value: 0.0,
            max_value: 1.0,
            auto_normalize: true,
            show_row_labels: false,
            show_col_labels: false,
            row_labels: None,
            col_labels: None,
        }
    }

    /// Create a heatmap from a 1D slice with given dimensions.
    pub fn from_flat(data: &[f32], rows: usize, cols: usize) -> Self {
        let mut grid = vec![vec![0.0; cols]; rows];
        for (i, &value) in data.iter().enumerate() {
            let row = i / cols;
            let col = i % cols;
            if row < rows && col < cols {
                grid[row][col] = value;
            }
        }
        Self::new(grid)
    }

    /// Set the color palette.
    pub fn palette(mut self, palette: HeatmapPalette) -> Self {
        self.palette = palette;
        self
    }

    /// Set the rendering style.
    pub fn style(mut self, style: HeatmapStyle) -> Self {
        self.style = style;
        self
    }

    /// Set cell width in characters.
    pub fn cell_width(mut self, width: u16) -> Self {
        self.cell_width = width.max(1);
        self
    }

    /// Set cell height in rows.
    pub fn cell_height(mut self, height: u16) -> Self {
        self.cell_height = height.max(1);
        self
    }

    /// Set the value range for normalization.
    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.min_value = min;
        self.max_value = max;
        self.auto_normalize = false;
        self
    }

    /// Enable auto-normalization based on data range.
    pub fn auto_normalize(mut self, enabled: bool) -> Self {
        self.auto_normalize = enabled;
        self
    }

    /// Show row labels.
    pub fn show_row_labels(mut self, show: bool) -> Self {
        self.show_row_labels = show;
        self
    }

    /// Show column labels.
    pub fn show_col_labels(mut self, show: bool) -> Self {
        self.show_col_labels = show;
        self
    }

    /// Set custom row labels.
    pub fn row_labels(mut self, labels: Vec<String>) -> Self {
        self.row_labels = Some(labels);
        self.show_row_labels = true;
        self
    }

    /// Set custom column labels.
    pub fn col_labels(mut self, labels: Vec<String>) -> Self {
        self.col_labels = Some(labels);
        self.show_col_labels = true;
        self
    }

    /// Get the number of rows.
    pub fn rows(&self) -> usize {
        self.data.len()
    }

    /// Get the number of columns.
    pub fn cols(&self) -> usize {
        self.data.first().map(|r| r.len()).unwrap_or(0)
    }

    /// Calculate auto-normalization bounds from data.
    fn calc_bounds(&self) -> (f32, f32) {
        let mut min = f32::MAX;
        let mut max = f32::MIN;
        for row in &self.data {
            for &value in row {
                min = min.min(value);
                max = max.max(value);
            }
        }
        if min == f32::MAX {
            (0.0, 1.0)
        } else {
            (min, max)
        }
    }

    /// Get effective bounds (auto-calculated or manual).
    fn effective_bounds(&self) -> (f32, f32) {
        if self.auto_normalize {
            self.calc_bounds()
        } else {
            (self.min_value, self.max_value)
        }
    }

    /// Get the color for a cell value.
    pub fn get_color(&self, value: f32) -> Color {
        self.get_color_with_bounds(value, self.effective_bounds())
    }

    /// Get color with pre-computed bounds (avoids repeated bounds calculation).
    fn get_color_with_bounds(&self, value: f32, (min, max): (f32, f32)) -> Color {
        let normalized = if max <= min {
            0.5
        } else {
            ((value - min) / (max - min)).clamp(0.0, 1.0)
        };

        self.palette.to_color(normalized)
    }

    /// Get the raw RGB data for GPU rendering.
    ///
    /// Returns a flat array of (r, g, b) tuples in row-major order.
    pub fn to_rgb_data(&self) -> Vec<(u8, u8, u8)> {
        let (min, max) = if self.auto_normalize {
            self.calc_bounds()
        } else {
            (self.min_value, self.max_value)
        };

        let mut result = Vec::with_capacity(self.rows() * self.cols());
        for row in &self.data {
            for &value in row {
                let normalized = if max <= min {
                    0.5
                } else {
                    ((value - min) / (max - min)).clamp(0.0, 1.0)
                };
                result.push(self.palette.to_rgb(normalized));
            }
        }
        result
    }
}

impl Default for Heatmap {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl From<Heatmap> for Node {
    fn from(heatmap: Heatmap) -> Self {
        let mut container = BoxNode::new().flex_direction(FlexDirection::Column);

        // Pre-compute bounds once for all cells (avoids O(n²) recalculation)
        let bounds = heatmap.effective_bounds();

        // Add column labels if requested
        if heatmap.show_col_labels && heatmap.cols() > 0 {
            let mut label_row = BoxNode::new().flex_direction(FlexDirection::Row);

            // Add spacer for row label column
            if heatmap.show_row_labels {
                label_row = label_row.child(TextNode::new("    "));
            }

            for col in 0..heatmap.cols() {
                let label = heatmap
                    .col_labels
                    .as_ref()
                    .and_then(|l| l.get(col))
                    .map(|s| s.as_str())
                    .unwrap_or_else(|| "");

                let text = format!("{:>width$}", label, width = heatmap.cell_width as usize);
                label_row = label_row.child(TextNode::new(text));
            }

            container = container.child(label_row);
        }

        // Render each row
        for (row_idx, row_data) in heatmap.data.iter().enumerate() {
            let mut row_box = BoxNode::new().flex_direction(FlexDirection::Row);

            // Add row label if requested
            if heatmap.show_row_labels {
                let label = heatmap
                    .row_labels
                    .as_ref()
                    .and_then(|l| l.get(row_idx))
                    .map(|s| s.as_str())
                    .unwrap_or_else(|| "");

                row_box = row_box.child(TextNode::new(format!("{:>3} ", label)));
            }

            // Add cells
            for &value in row_data {
                let color = heatmap.get_color_with_bounds(value, bounds);
                let cell_text = match heatmap.style {
                    HeatmapStyle::Background => " ".repeat(heatmap.cell_width as usize),
                    HeatmapStyle::Blocks => "█".repeat(heatmap.cell_width as usize),
                    HeatmapStyle::HalfBlocks => "▀".repeat(heatmap.cell_width as usize),
                    HeatmapStyle::Braille => "⣿".repeat(heatmap.cell_width as usize),
                };

                let cell = match heatmap.style {
                    HeatmapStyle::Background => TextNode::new(cell_text).bg(color),
                    _ => TextNode::new(cell_text).color(color),
                };

                row_box = row_box.child(cell);
            }

            container = container.child(row_box);
        }

        container.into()
    }
}

impl AdaptiveComponent for Heatmap {
    fn render_for_tier(&self, tier: RenderTier) -> Node {
        match tier {
            RenderTier::Tier0Fallback => self.render_tier0(),
            RenderTier::Tier1Ansi => self.render_tier1(),
            RenderTier::Tier2Retained | RenderTier::Tier3Gpu => self.clone().into(),
        }
    }

    fn tier_features(&self) -> TierFeatures {
        TierFeatures::new("Heatmap")
            .tier0("Text summary with min/max/mean stats")
            .tier1("ASCII density grid (` .:;oO@#`)")
            .tier2("Unicode blocks with 24-bit true color")
            .tier3("GPU-accelerated smooth gradients")
    }

    fn minimum_tier(&self) -> Option<RenderTier> {
        None // Works at all tiers
    }
}

impl Heatmap {
    /// Render Tier 0: Text-only summary.
    fn render_tier0(&self) -> Node {
        let (min, max) = self.calc_bounds();
        let mean = self.calculate_mean();
        let (rows, cols) = (self.rows(), self.cols());

        Tier0Fallback::new("Heatmap")
            .stat("size", format!("{}x{}", rows, cols))
            .stat("min", format!("{:.2}", min))
            .stat("max", format!("{:.2}", max))
            .stat("mean", format!("{:.2}", mean))
            .into()
    }

    /// Render Tier 1: ASCII density characters.
    fn render_tier1(&self) -> Node {
        // ASCII density ramp from low to high
        const DENSITY_CHARS: &[char] = &[' ', '.', ':', ';', 'o', 'O', '@', '#'];

        let mut container = BoxNode::new().flex_direction(FlexDirection::Column);
        let bounds = self.effective_bounds();
        let (min, max) = bounds;
        let range = max - min;

        for row_data in &self.data {
            let row_str: String = row_data
                .iter()
                .map(|&value| {
                    let normalized = if range <= 0.0 {
                        0.5
                    } else {
                        ((value - min) / range).clamp(0.0, 1.0)
                    };
                    let idx = (normalized * (DENSITY_CHARS.len() - 1) as f32).round() as usize;
                    DENSITY_CHARS[idx.min(DENSITY_CHARS.len() - 1)]
                })
                .flat_map(|c| std::iter::repeat(c).take(self.cell_width as usize))
                .collect();

            container = container.child(TextNode::new(row_str));
        }

        container.into()
    }

    /// Calculate the mean value of all data points.
    fn calculate_mean(&self) -> f32 {
        let mut sum = 0.0;
        let mut count = 0;
        for row in &self.data {
            for &value in row {
                sum += value;
                count += 1;
            }
        }
        if count == 0 {
            0.0
        } else {
            sum / count as f32
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_heatmap_new() {
        let data = vec![vec![0.0, 0.5, 1.0], vec![0.2, 0.7, 0.9]];
        let heatmap = Heatmap::new(data);
        assert_eq!(heatmap.rows(), 2);
        assert_eq!(heatmap.cols(), 3);
    }

    #[test]
    fn test_heatmap_from_flat() {
        let data = vec![0.0, 0.1, 0.2, 0.3, 0.4, 0.5];
        let heatmap = Heatmap::from_flat(&data, 2, 3);
        assert_eq!(heatmap.rows(), 2);
        assert_eq!(heatmap.cols(), 3);
    }

    #[test]
    fn test_palette_grayscale() {
        let palette = HeatmapPalette::Grayscale;
        let (r, g, b) = palette.to_rgb(0.0);
        assert_eq!((r, g, b), (0, 0, 0));

        let (r, g, b) = palette.to_rgb(1.0);
        assert_eq!((r, g, b), (255, 255, 255));

        let (r, g, b) = palette.to_rgb(0.5);
        assert!(r == g && g == b);
    }

    #[test]
    fn test_palette_heat() {
        let palette = HeatmapPalette::Heat;

        // At 0.0, should be black
        let (r, g, b) = palette.to_rgb(0.0);
        assert_eq!((r, g, b), (0, 0, 0));

        // At 1.0, should be white
        let (r, g, b) = palette.to_rgb(1.0);
        assert_eq!((r, g, b), (255, 255, 255));
    }

    #[test]
    fn test_heatmap_get_color() {
        let data = vec![vec![0.0, 0.5, 1.0]];
        let heatmap = Heatmap::new(data).palette(HeatmapPalette::Grayscale);

        let color = heatmap.get_color(0.0);
        assert!(matches!(color, Color::Rgb(0, 0, 0)));

        let color = heatmap.get_color(1.0);
        assert!(matches!(color, Color::Rgb(255, 255, 255)));
    }

    #[test]
    fn test_heatmap_to_rgb_data() {
        let data = vec![vec![0.0, 1.0]];
        let heatmap = Heatmap::new(data).palette(HeatmapPalette::Grayscale);

        let rgb_data = heatmap.to_rgb_data();
        assert_eq!(rgb_data.len(), 2);
        assert_eq!(rgb_data[0], (0, 0, 0));
        assert_eq!(rgb_data[1], (255, 255, 255));
    }

    #[test]
    fn test_heatmap_custom_range() {
        let data = vec![vec![50.0, 100.0, 150.0]];
        let heatmap = Heatmap::new(data)
            .range(0.0, 200.0)
            .palette(HeatmapPalette::Grayscale);

        let rgb_data = heatmap.to_rgb_data();
        // 50/200 = 0.25 → ~64
        // 100/200 = 0.5 → ~128
        // 150/200 = 0.75 → ~191
        assert!(rgb_data[0].0 < rgb_data[1].0);
        assert!(rgb_data[1].0 < rgb_data[2].0);
    }

    #[test]
    fn test_heatmap_to_node() {
        let data = vec![vec![0.0, 0.5], vec![0.5, 1.0]];
        let heatmap = Heatmap::new(data);
        let node: Node = heatmap.into();
        assert!(matches!(node, Node::Box(_)));
    }

    #[test]
    fn test_heatmap_styles() {
        let data = vec![vec![0.5]];

        for style in [
            HeatmapStyle::Background,
            HeatmapStyle::Blocks,
            HeatmapStyle::HalfBlocks,
            HeatmapStyle::Braille,
        ] {
            let heatmap = Heatmap::new(data.clone()).style(style);
            let _node: Node = heatmap.into();
        }
    }

    #[test]
    fn test_all_palettes() {
        for palette in [
            HeatmapPalette::Grayscale,
            HeatmapPalette::Heat,
            HeatmapPalette::Cool,
            HeatmapPalette::Viridis,
            HeatmapPalette::Plasma,
            HeatmapPalette::RedGreen,
        ] {
            // Should not panic for any value
            let _ = palette.to_rgb(0.0);
            let _ = palette.to_rgb(0.5);
            let _ = palette.to_rgb(1.0);
            let _ = palette.to_rgb(-1.0); // Out of range
            let _ = palette.to_rgb(2.0); // Out of range
        }
    }

    #[test]
    fn test_heatmap_adaptive_tier0() {
        let data = vec![vec![0.0, 0.5, 1.0], vec![0.2, 0.7, 0.9]];
        let heatmap = Heatmap::new(data);

        let node = heatmap.render_for_tier(RenderTier::Tier0Fallback);
        // Should render as text node with summary stats
        assert!(matches!(node, Node::Text(_)));
    }

    #[test]
    fn test_heatmap_adaptive_tier1() {
        let data = vec![vec![0.0, 0.5, 1.0], vec![0.2, 0.7, 0.9]];
        let heatmap = Heatmap::new(data);

        let node = heatmap.render_for_tier(RenderTier::Tier1Ansi);
        // Should render as box with ASCII characters
        assert!(matches!(node, Node::Box(_)));
    }

    #[test]
    fn test_heatmap_adaptive_tier2() {
        let data = vec![vec![0.0, 0.5, 1.0], vec![0.2, 0.7, 0.9]];
        let heatmap = Heatmap::new(data);

        let node = heatmap.render_for_tier(RenderTier::Tier2Retained);
        // Should render same as default (Unicode with colors)
        assert!(matches!(node, Node::Box(_)));
    }

    #[test]
    fn test_heatmap_adaptive_all_tiers() {
        let data = vec![vec![0.0, 0.5, 1.0]];
        let heatmap = Heatmap::new(data);

        // Should render without panic at all tiers
        for tier in [
            RenderTier::Tier0Fallback,
            RenderTier::Tier1Ansi,
            RenderTier::Tier2Retained,
            RenderTier::Tier3Gpu,
        ] {
            let _node = heatmap.render_for_tier(tier);
        }
    }

    #[test]
    fn test_heatmap_tier_features() {
        let heatmap = Heatmap::new(vec![vec![0.5]]);
        let features = heatmap.tier_features();

        assert_eq!(features.name, Some("Heatmap"));
        assert!(features.tier0_description.is_some());
        assert!(features.tier1_description.is_some());
        assert!(features.tier2_description.is_some());
        assert!(features.tier3_description.is_some());
        assert!(features.gpu_enhanced);
    }

    #[test]
    fn test_heatmap_supports_all_tiers() {
        let heatmap = Heatmap::new(vec![vec![0.5]]);

        // Heatmap should work at all tiers
        assert!(heatmap.supports_tier(RenderTier::Tier0Fallback));
        assert!(heatmap.supports_tier(RenderTier::Tier1Ansi));
        assert!(heatmap.supports_tier(RenderTier::Tier2Retained));
        assert!(heatmap.supports_tier(RenderTier::Tier3Gpu));
    }

    #[test]
    fn test_heatmap_calculate_mean() {
        let data = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let heatmap = Heatmap::new(data);

        // Mean of [1,2,3,4,5,6] = 21/6 = 3.5
        assert!((heatmap.calculate_mean() - 3.5).abs() < 0.001);
    }
}
