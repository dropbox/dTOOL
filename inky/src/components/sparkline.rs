//! Sparkline component - inline charts.
//!
//! Renders a series of values as a compact inline chart using
//! Unicode block characters or braille patterns.
//!
//! # Performance
//!
//! Uses `VecDeque` for O(1) data window operations when pushing new values.
//!
//! # Adaptive Rendering
//!
//! `Sparkline` implements [`AdaptiveComponent`] for graceful degradation:
//!
//! | Tier | Rendering |
//! |------|-----------|
//! | 0 (Fallback) | Text summary: min, max, current, trend |
//! | 1 (ANSI) | ASCII graph characters (`_.-=#`) |
//! | 2 (Retained) | Unicode blocks with colors |
//! | 3 (GPU) | GPU-accelerated rendering (same as Tier 2 for now) |

use crate::components::adaptive::{AdaptiveComponent, Tier0Fallback, TierFeatures};
use crate::node::{BoxNode, Node, TextNode};
use crate::style::{Color, FlexDirection};
use crate::terminal::RenderTier;
use std::collections::VecDeque;

/// Sparkline rendering style.
#[derive(Debug, Clone, Copy, Default)]
pub enum SparklineStyle {
    /// Use vertical block characters: ▁▂▃▄▅▆▇█
    #[default]
    Blocks,
    /// Use braille patterns for higher resolution
    Braille,
    /// Use ASCII characters
    Ascii,
    /// Use line drawing characters
    Line,
}

impl SparklineStyle {
    /// Get the characters used for this style.
    pub fn chars(&self) -> &'static [char] {
        match self {
            SparklineStyle::Blocks => &['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'],
            SparklineStyle::Braille => &['⠀', '⣀', '⣤', '⣶', '⣿'],
            SparklineStyle::Ascii => &['_', '.', '-', '=', '#'],
            SparklineStyle::Line => &['╸', '╺', '━', '┃', '┓', '┏', '┛', '┗'],
        }
    }

    /// Convert a normalized value (0.0-1.0) to a character.
    pub fn char_for_value(&self, value: f32) -> char {
        let chars = self.chars();
        let v = value.clamp(0.0, 1.0);
        let idx = (v * (chars.len() - 1) as f32).round() as usize;
        chars[idx.min(chars.len() - 1)]
    }
}

/// Inline sparkline chart component.
///
/// # Example
///
/// ```ignore
/// use inky::prelude::*;
///
/// let data = vec![1.0, 2.0, 3.0, 4.0, 3.0, 2.0, 1.0];
///
/// let sparkline = Sparkline::new(data)
///     .color(Color::Green)
///     .style(SparklineStyle::Blocks);
/// ```
#[derive(Debug, Clone)]
pub struct Sparkline {
    /// Data values stored in a VecDeque for O(1) front removal.
    data: VecDeque<f32>,
    /// Rendering style.
    style: SparklineStyle,
    /// Foreground color.
    color: Option<Color>,
    /// Background color.
    bg_color: Option<Color>,
    /// Minimum value for normalization.
    min_value: Option<f32>,
    /// Maximum value for normalization.
    max_value: Option<f32>,
    /// Maximum width (number of data points to show).
    max_width: Option<usize>,
    /// Label to show before the sparkline.
    label: Option<String>,
    /// Show current value.
    show_value: bool,
    /// Show min/max values.
    show_range: bool,
}

impl Sparkline {
    /// Create a new sparkline with the given data.
    pub fn new(data: Vec<f32>) -> Self {
        Self {
            data: VecDeque::from(data),
            style: SparklineStyle::default(),
            color: None,
            bg_color: None,
            min_value: None,
            max_value: None,
            max_width: None,
            label: None,
            show_value: false,
            show_range: false,
        }
    }

    /// Create an empty sparkline.
    pub fn empty() -> Self {
        Self {
            data: VecDeque::new(),
            style: SparklineStyle::default(),
            color: None,
            bg_color: None,
            min_value: None,
            max_value: None,
            max_width: None,
            label: None,
            show_value: false,
            show_range: false,
        }
    }

    /// Set the rendering style.
    pub fn style(mut self, style: SparklineStyle) -> Self {
        self.style = style;
        self
    }

    /// Set the foreground color.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Set the background color.
    pub fn bg_color(mut self, color: Color) -> Self {
        self.bg_color = Some(color);
        self
    }

    /// Set the value range for normalization.
    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.min_value = Some(min);
        self.max_value = Some(max);
        self
    }

    /// Set minimum value for normalization.
    pub fn min(mut self, min: f32) -> Self {
        self.min_value = Some(min);
        self
    }

    /// Set maximum value for normalization.
    pub fn max(mut self, max: f32) -> Self {
        self.max_value = Some(max);
        self
    }

    /// Set maximum width (truncates/windows data).
    pub fn max_width(mut self, width: usize) -> Self {
        self.max_width = Some(width);
        self
    }

    /// Set label to show before the sparkline.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Show the current (last) value after the sparkline.
    pub fn show_value(mut self, show: bool) -> Self {
        self.show_value = show;
        self
    }

    /// Show min/max range after the sparkline.
    pub fn show_range(mut self, show: bool) -> Self {
        self.show_range = show;
        self
    }

    /// Push a new value to the sparkline.
    ///
    /// If `max_width` is set and exceeded, old values are removed from the front.
    /// This is O(1) thanks to VecDeque.
    pub fn push(&mut self, value: f32) {
        self.data.push_back(value);
        // Trim to max_width if set - O(1) pop_front with VecDeque
        if let Some(max_width) = self.max_width {
            while self.data.len() > max_width {
                self.data.pop_front();
            }
        }
    }

    /// Get the data values as a contiguous slice.
    ///
    /// Note: This may need to make the VecDeque contiguous internally.
    pub fn data(&self) -> Vec<f32> {
        self.data.iter().copied().collect()
    }

    /// Get the current (last) value.
    pub fn current(&self) -> Option<f32> {
        self.data.back().copied()
    }

    /// Get the minimum value in the data.
    pub fn data_min(&self) -> Option<f32> {
        self.data.iter().copied().reduce(f32::min)
    }

    /// Get the maximum value in the data.
    pub fn data_max(&self) -> Option<f32> {
        self.data.iter().copied().reduce(f32::max)
    }

    /// Get the number of data points.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the sparkline is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Render the sparkline to a string.
    pub fn render(&self) -> String {
        if self.data.is_empty() {
            return String::new();
        }

        // Determine the data window to render
        let data_len = self.data.len();
        let skip = self
            .max_width
            .map(|max_width| data_len.saturating_sub(max_width))
            .unwrap_or(0);

        let min = self.min_value.unwrap_or_else(|| {
            self.data
                .iter()
                .skip(skip)
                .copied()
                .reduce(f32::min)
                .unwrap_or(0.0)
        });
        let max = self.max_value.unwrap_or_else(|| {
            self.data
                .iter()
                .skip(skip)
                .copied()
                .reduce(f32::max)
                .unwrap_or(1.0)
        });

        let range = max - min;
        if range <= 0.0 {
            // All values are the same
            let count = if let Some(max_width) = self.max_width {
                data_len.min(max_width)
            } else {
                data_len
            };
            return self.style.char_for_value(0.5).to_string().repeat(count);
        }

        self.data
            .iter()
            .skip(skip)
            .map(|&v| {
                let normalized = (v - min) / range;
                self.style.char_for_value(normalized)
            })
            .collect()
    }
}

impl Default for Sparkline {
    fn default() -> Self {
        Self::empty()
    }
}

impl From<Sparkline> for Node {
    fn from(sparkline: Sparkline) -> Self {
        let mut container = BoxNode::new().flex_direction(FlexDirection::Row).gap(1.0);

        // Add label if present
        if let Some(label) = &sparkline.label {
            container = container.child(TextNode::new(label));
        }

        // Render the sparkline
        let rendered = sparkline.render();
        let mut text_node = TextNode::new(rendered);

        if let Some(color) = sparkline.color {
            text_node = text_node.color(color);
        }
        if let Some(bg) = sparkline.bg_color {
            text_node = text_node.bg(bg);
        }

        container = container.child(text_node);

        // Add value display if requested
        if sparkline.show_value {
            if let Some(current) = sparkline.current() {
                container = container.child(TextNode::new(format!("{:.1}", current)));
            }
        }

        // Add range display if requested
        if sparkline.show_range {
            if let (Some(min), Some(max)) = (sparkline.data_min(), sparkline.data_max()) {
                container = container.child(TextNode::new(format!("[{:.1}-{:.1}]", min, max)));
            }
        }

        container.into()
    }
}

impl AdaptiveComponent for Sparkline {
    fn render_for_tier(&self, tier: RenderTier) -> Node {
        match tier {
            RenderTier::Tier0Fallback => self.render_tier0(),
            RenderTier::Tier1Ansi => self.render_tier1(),
            RenderTier::Tier2Retained | RenderTier::Tier3Gpu => self.clone().into(),
        }
    }

    fn tier_features(&self) -> TierFeatures {
        TierFeatures::new("Sparkline")
            .tier0("Text summary with min/max/current/trend")
            .tier1("ASCII graph characters (`_.-=#`)")
            .tier2("Unicode block characters with colors")
            .tier3("GPU-accelerated smooth line rendering")
    }

    fn minimum_tier(&self) -> Option<RenderTier> {
        None // Works at all tiers
    }
}

impl Sparkline {
    /// Render Tier 0: Text-only summary.
    fn render_tier0(&self) -> Node {
        let mut fallback = Tier0Fallback::new("Sparkline");

        if let Some(current) = self.current() {
            fallback = fallback.stat("current", format!("{:.2}", current));
        }
        if let Some(min) = self.data_min() {
            fallback = fallback.stat("min", format!("{:.2}", min));
        }
        if let Some(max) = self.data_max() {
            fallback = fallback.stat("max", format!("{:.2}", max));
        }

        // Add trend indicator
        let trend = self.calculate_trend();
        let trend_str = if trend > 0.1 {
            "up"
        } else if trend < -0.1 {
            "down"
        } else {
            "stable"
        };
        fallback = fallback.stat("trend", trend_str);

        fallback.into()
    }

    /// Render Tier 1: ASCII characters.
    fn render_tier1(&self) -> Node {
        // Use ASCII style for Tier 1
        let ascii_sparkline = Self {
            style: SparklineStyle::Ascii,
            ..self.clone()
        };
        ascii_sparkline.into()
    }

    /// Calculate trend direction (-1.0 to 1.0).
    fn calculate_trend(&self) -> f32 {
        if self.data.len() < 2 {
            return 0.0;
        }

        // Compare first half average to second half average
        let mid = self.data.len() / 2;
        let first_half: f32 = self.data.iter().take(mid).sum::<f32>() / mid as f32;
        let second_half: f32 =
            self.data.iter().skip(mid).sum::<f32>() / (self.data.len() - mid) as f32;

        let range = self.data_max().unwrap_or(1.0) - self.data_min().unwrap_or(0.0);
        if range <= 0.0 {
            0.0
        } else {
            ((second_half - first_half) / range).clamp(-1.0, 1.0)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_sparkline_new() {
        let sparkline = Sparkline::new(vec![1.0, 2.0, 3.0]);
        assert_eq!(sparkline.len(), 3);
    }

    #[test]
    fn test_sparkline_empty() {
        let sparkline = Sparkline::empty();
        assert!(sparkline.is_empty());
        assert_eq!(sparkline.render(), "");
    }

    #[test]
    fn test_sparkline_render() {
        let sparkline = Sparkline::new(vec![0.0, 0.5, 1.0]);
        let rendered = sparkline.render();
        assert_eq!(rendered.chars().count(), 3);
    }

    #[test]
    fn test_sparkline_render_blocks() {
        let sparkline = Sparkline::new(vec![0.0, 0.5, 1.0]).style(SparklineStyle::Blocks);
        let rendered = sparkline.render();
        assert!(rendered.contains('▁')); // Low
        assert!(rendered.contains('█')); // High
    }

    #[test]
    fn test_sparkline_current() {
        let sparkline = Sparkline::new(vec![1.0, 2.0, 3.0]);
        assert_eq!(sparkline.current(), Some(3.0));
    }

    #[test]
    fn test_sparkline_data_range() {
        let sparkline = Sparkline::new(vec![1.0, 5.0, 3.0]);
        assert_eq!(sparkline.data_min(), Some(1.0));
        assert_eq!(sparkline.data_max(), Some(5.0));
    }

    #[test]
    fn test_sparkline_push() {
        let mut sparkline = Sparkline::new(vec![1.0, 2.0]).max_width(3);
        sparkline.push(3.0);
        sparkline.push(4.0);
        assert_eq!(sparkline.len(), 3);
        assert_eq!(sparkline.data(), vec![2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_sparkline_custom_range() {
        let sparkline = Sparkline::new(vec![50.0, 100.0, 150.0]).range(0.0, 200.0);
        let rendered = sparkline.render();
        assert_eq!(rendered.chars().count(), 3);
    }

    #[test]
    fn test_sparkline_max_width() {
        let sparkline = Sparkline::new(vec![1.0, 2.0, 3.0, 4.0, 5.0]).max_width(3);
        let rendered = sparkline.render();
        assert_eq!(rendered.chars().count(), 3);
    }

    #[test]
    fn test_sparkline_to_node() {
        let sparkline = Sparkline::new(vec![1.0, 2.0, 3.0])
            .label("CPU:")
            .show_value(true);
        let node: Node = sparkline.into();
        assert!(matches!(node, Node::Box(_)));
    }

    #[test]
    fn test_sparkline_styles() {
        let data = vec![0.0, 0.25, 0.5, 0.75, 1.0];

        for style in [
            SparklineStyle::Blocks,
            SparklineStyle::Braille,
            SparklineStyle::Ascii,
            SparklineStyle::Line,
        ] {
            let sparkline = Sparkline::new(data.clone()).style(style);
            let rendered = sparkline.render();
            assert_eq!(rendered.chars().count(), 5);
        }
    }

    #[test]
    fn test_sparkline_constant_values() {
        // All same values should render without panic
        let sparkline = Sparkline::new(vec![5.0, 5.0, 5.0]);
        let rendered = sparkline.render();
        assert_eq!(rendered.chars().count(), 3);
    }

    #[test]
    fn test_sparkline_adaptive_tier0() {
        let sparkline = Sparkline::new(vec![1.0, 2.0, 3.0, 4.0, 5.0]);

        let node = sparkline.render_for_tier(RenderTier::Tier0Fallback);
        // Should render as text node with summary stats
        assert!(matches!(node, Node::Text(_)));
    }

    #[test]
    fn test_sparkline_adaptive_tier1() {
        let sparkline = Sparkline::new(vec![1.0, 2.0, 3.0]);

        let node = sparkline.render_for_tier(RenderTier::Tier1Ansi);
        // Should render as box with ASCII characters
        assert!(matches!(node, Node::Box(_)));
    }

    #[test]
    fn test_sparkline_adaptive_tier2() {
        let sparkline = Sparkline::new(vec![1.0, 2.0, 3.0]);

        let node = sparkline.render_for_tier(RenderTier::Tier2Retained);
        // Should render same as default (Unicode blocks)
        assert!(matches!(node, Node::Box(_)));
    }

    #[test]
    fn test_sparkline_adaptive_all_tiers() {
        let sparkline = Sparkline::new(vec![1.0, 2.0, 3.0, 4.0, 5.0]);

        // Should render without panic at all tiers
        for tier in [
            RenderTier::Tier0Fallback,
            RenderTier::Tier1Ansi,
            RenderTier::Tier2Retained,
            RenderTier::Tier3Gpu,
        ] {
            let _node = sparkline.render_for_tier(tier);
        }
    }

    #[test]
    fn test_sparkline_tier_features() {
        let sparkline = Sparkline::new(vec![1.0, 2.0, 3.0]);
        let features = sparkline.tier_features();

        assert_eq!(features.name, Some("Sparkline"));
        assert!(features.tier0_description.is_some());
        assert!(features.tier1_description.is_some());
        assert!(features.tier2_description.is_some());
        assert!(features.tier3_description.is_some());
        assert!(features.gpu_enhanced);
    }

    #[test]
    fn test_sparkline_calculate_trend_up() {
        // Rising trend: 1,2,3,4,5
        let sparkline = Sparkline::new(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        let trend = sparkline.calculate_trend();
        assert!(trend > 0.0, "Expected positive trend, got {}", trend);
    }

    #[test]
    fn test_sparkline_calculate_trend_down() {
        // Falling trend: 5,4,3,2,1
        let sparkline = Sparkline::new(vec![5.0, 4.0, 3.0, 2.0, 1.0]);
        let trend = sparkline.calculate_trend();
        assert!(trend < 0.0, "Expected negative trend, got {}", trend);
    }

    #[test]
    fn test_sparkline_calculate_trend_stable() {
        // Stable trend: all same values
        let sparkline = Sparkline::new(vec![3.0, 3.0, 3.0, 3.0]);
        let trend = sparkline.calculate_trend();
        assert!(
            trend.abs() < 0.1,
            "Expected stable trend near 0, got {}",
            trend
        );
    }

    #[test]
    fn test_sparkline_supports_all_tiers() {
        let sparkline = Sparkline::new(vec![1.0, 2.0]);

        // Sparkline should work at all tiers
        assert!(sparkline.supports_tier(RenderTier::Tier0Fallback));
        assert!(sparkline.supports_tier(RenderTier::Tier1Ansi));
        assert!(sparkline.supports_tier(RenderTier::Tier2Retained));
        assert!(sparkline.supports_tier(RenderTier::Tier3Gpu));
    }
}
