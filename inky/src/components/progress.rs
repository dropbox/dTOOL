//! Progress component - progress bar.
//!
//! # Adaptive Rendering
//!
//! `Progress` implements [`AdaptiveComponent`] for graceful degradation:
//!
//! | Tier | Rendering |
//! |------|-----------|
//! | 0 (Fallback) | Text only: "75%" or "75/100" |
//! | 1 (ANSI) | ASCII bar: `[========--------]` |
//! | 2 (Retained) | Unicode bar with colors: `▓▓▓▓▓▓▓▓░░░░` |
//! | 3 (GPU) | GPU-accelerated smooth animation (same as Tier 2 for now) |

use crate::components::adaptive::{AdaptiveComponent, Tier0Fallback, TierFeatures};
use crate::node::{BoxNode, Node, TextNode};
use crate::style::{Color, FlexDirection};
use crate::terminal::RenderTier;

/// Progress bar style.
#[derive(Debug, Clone, Copy, Default)]
pub enum ProgressStyle {
    /// Block characters: ████████░░░░░░░░
    #[default]
    Block,
    /// ASCII bar: [========--------]
    Ascii,
    /// Unicode bar: ▓▓▓▓▓▓▓▓░░░░░░░░
    Unicode,
    /// Thin bar: ━━━━━━━━────────
    Thin,
    /// Braille dots: ⣿⣿⣿⣿⣿⣿⡀⠀⠀⠀⠀⠀
    Braille,
}

impl ProgressStyle {
    /// Get the filled character for this style.
    pub fn filled(&self) -> &'static str {
        match self {
            ProgressStyle::Block => "█",
            ProgressStyle::Ascii => "=",
            ProgressStyle::Unicode => "▓",
            ProgressStyle::Thin => "━",
            ProgressStyle::Braille => "⣿",
        }
    }

    /// Get the empty character for this style.
    pub fn empty(&self) -> &'static str {
        match self {
            ProgressStyle::Block => "░",
            ProgressStyle::Ascii => "-",
            ProgressStyle::Unicode => "░",
            ProgressStyle::Thin => "─",
            ProgressStyle::Braille => "⠀",
        }
    }

    /// Get optional brackets for this style.
    pub fn brackets(&self) -> Option<(&'static str, &'static str)> {
        match self {
            ProgressStyle::Ascii => Some(("[", "]")),
            _ => None,
        }
    }
}

/// Progress bar component.
///
/// # Example
///
/// ```ignore
/// use inky::prelude::*;
///
/// let progress = Progress::new()
///     .progress(0.75)
///     .width(40)
///     .show_percentage(true);
/// ```
#[derive(Debug, Clone)]
pub struct Progress {
    /// Current progress (0.0 to 1.0).
    progress: f32,
    /// Width of the bar in characters.
    width: u16,
    /// Style of the progress bar.
    style: ProgressStyle,
    /// Whether to show percentage text.
    show_percentage: bool,
    /// Whether to show value text (e.g., "75/100").
    show_value: bool,
    /// Total value for show_value display.
    total: Option<u64>,
    /// Current value for show_value display.
    current: Option<u64>,
    /// Filled portion color.
    filled_color: Option<Color>,
    /// Empty portion color.
    empty_color: Option<Color>,
    /// Label to show before the bar.
    label: Option<String>,
}

impl Progress {
    /// Create a new progress bar.
    pub fn new() -> Self {
        Self {
            progress: 0.0,
            width: 20,
            style: ProgressStyle::default(),
            show_percentage: false,
            show_value: false,
            total: None,
            current: None,
            filled_color: None,
            empty_color: Some(Color::BrightBlack),
            label: None,
        }
    }

    /// Set progress value (0.0 to 1.0).
    pub fn progress(mut self, value: f32) -> Self {
        self.progress = value.clamp(0.0, 1.0);
        self
    }

    /// Set progress from current and total values.
    pub fn value(mut self, current: u64, total: u64) -> Self {
        self.current = Some(current);
        self.total = Some(total);
        self.progress = if total > 0 {
            (current as f32 / total as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self
    }

    /// Set the width of the bar.
    pub fn width(mut self, width: u16) -> Self {
        self.width = width;
        self
    }

    /// Set progress bar style.
    pub fn style(mut self, style: ProgressStyle) -> Self {
        self.style = style;
        self
    }

    /// Show percentage after the bar.
    pub fn show_percentage(mut self, show: bool) -> Self {
        self.show_percentage = show;
        self
    }

    /// Show value (current/total) after the bar.
    pub fn show_value(mut self, show: bool) -> Self {
        self.show_value = show;
        self
    }

    /// Set filled portion color.
    pub fn filled_color(mut self, color: Color) -> Self {
        self.filled_color = Some(color);
        self
    }

    /// Set empty portion color.
    pub fn empty_color(mut self, color: Color) -> Self {
        self.empty_color = Some(color);
        self
    }

    /// Set label before the bar.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Get current progress value.
    pub fn get_progress(&self) -> f32 {
        self.progress
    }

    /// Get percentage (0-100).
    pub fn percentage(&self) -> u8 {
        (self.progress * 100.0).round() as u8
    }

    /// Render the progress bar to a string.
    pub fn render_bar(&self) -> String {
        let (left_bracket, right_bracket) = self.style.brackets().unwrap_or(("", ""));
        let bracket_width = left_bracket.len() + right_bracket.len();
        let bar_width = (self.width as usize).saturating_sub(bracket_width);

        let filled_count = ((bar_width as f32) * self.progress).round() as usize;
        let empty_count = bar_width.saturating_sub(filled_count);

        let filled_str = self.style.filled();
        let empty_str = self.style.empty();

        // Single allocation with exact capacity
        let capacity =
            bracket_width + filled_count * filled_str.len() + empty_count * empty_str.len();
        let mut bar = String::with_capacity(capacity);

        bar.push_str(left_bracket);
        for _ in 0..filled_count {
            bar.push_str(filled_str);
        }
        for _ in 0..empty_count {
            bar.push_str(empty_str);
        }
        bar.push_str(right_bracket);

        bar
    }
}

impl Default for Progress {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Progress> for Node {
    fn from(progress: Progress) -> Self {
        let mut container = BoxNode::new().flex_direction(FlexDirection::Row).gap(1.0);

        // Add label if present
        if let Some(label) = &progress.label {
            container = container.child(TextNode::new(label));
        }

        // Calculate bar dimensions
        let filled_count = ((progress.width as f32) * progress.progress).round() as usize;

        // Create bar node - we need to color filled and empty portions differently
        // For simplicity, we'll use two separate text nodes
        let (left_bracket, right_bracket) = progress.style.brackets().unwrap_or(("", ""));
        let bracket_width = left_bracket.len() + right_bracket.len();
        let bar_width = (progress.width as usize).saturating_sub(bracket_width);
        let empty_count = bar_width.saturating_sub(filled_count);

        let mut bar_box = BoxNode::new().flex_direction(FlexDirection::Row);

        if !left_bracket.is_empty() {
            bar_box = bar_box.child(TextNode::new(left_bracket));
        }

        // Filled portion
        if filled_count > 0 {
            let filled_text = progress.style.filled().repeat(filled_count);
            let mut filled_node = TextNode::new(filled_text);
            if let Some(color) = progress.filled_color {
                filled_node = filled_node.color(color);
            }
            bar_box = bar_box.child(filled_node);
        }

        // Empty portion
        if empty_count > 0 {
            let empty_text = progress.style.empty().repeat(empty_count);
            let mut empty_node = TextNode::new(empty_text);
            if let Some(color) = progress.empty_color {
                empty_node = empty_node.color(color);
            }
            bar_box = bar_box.child(empty_node);
        }

        if !right_bracket.is_empty() {
            bar_box = bar_box.child(TextNode::new(right_bracket));
        }

        container = container.child(bar_box);

        // Add percentage or value display
        if progress.show_percentage {
            let pct = format!("{:>3}%", progress.percentage());
            container = container.child(TextNode::new(pct));
        } else if progress.show_value {
            if let (Some(current), Some(total)) = (progress.current, progress.total) {
                let value = format!("{}/{}", current, total);
                container = container.child(TextNode::new(value));
            }
        }

        container.into()
    }
}

impl AdaptiveComponent for Progress {
    fn render_for_tier(&self, tier: RenderTier) -> Node {
        match tier {
            RenderTier::Tier0Fallback => self.render_tier0(),
            RenderTier::Tier1Ansi => self.render_tier1(),
            RenderTier::Tier2Retained | RenderTier::Tier3Gpu => self.clone().into(),
        }
    }

    fn tier_features(&self) -> TierFeatures {
        TierFeatures::new("Progress")
            .tier0("Text percentage only (e.g., '75%')")
            .tier1("ASCII bar with brackets ([====----])")
            .tier2("Unicode blocks with 24-bit colors")
            .tier3("GPU-accelerated smooth animation")
    }

    fn minimum_tier(&self) -> Option<RenderTier> {
        None // Works at all tiers
    }
}

impl Progress {
    /// Render Tier 0: Text-only percentage.
    fn render_tier0(&self) -> Node {
        let mut fallback = Tier0Fallback::new("Progress");

        if let (Some(current), Some(total)) = (self.current, self.total) {
            fallback = fallback.stat("value", format!("{}/{}", current, total));
        } else {
            fallback = fallback.stat("percent", format!("{}%", self.percentage()));
        }

        if let Some(ref label) = self.label {
            fallback = Tier0Fallback::new(label.clone())
                .stat("percent", format!("{}%", self.percentage()));
        }

        fallback.into()
    }

    /// Render Tier 1: ASCII bar.
    fn render_tier1(&self) -> Node {
        // Use ASCII style for Tier 1
        let ascii_progress = Self {
            style: ProgressStyle::Ascii,
            filled_color: None, // No colors in Tier 1
            empty_color: None,
            ..self.clone()
        };
        ascii_progress.into()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_new() {
        let progress = Progress::new();
        assert_eq!(progress.get_progress(), 0.0);
        assert_eq!(progress.percentage(), 0);
    }

    #[test]
    fn test_progress_value() {
        let progress = Progress::new().progress(0.75);
        assert_eq!(progress.get_progress(), 0.75);
        assert_eq!(progress.percentage(), 75);
    }

    #[test]
    fn test_progress_clamp() {
        let progress = Progress::new().progress(1.5);
        assert_eq!(progress.get_progress(), 1.0);

        let progress = Progress::new().progress(-0.5);
        assert_eq!(progress.get_progress(), 0.0);
    }

    #[test]
    fn test_progress_from_value() {
        let progress = Progress::new().value(25, 100);
        assert_eq!(progress.get_progress(), 0.25);
        assert_eq!(progress.percentage(), 25);
    }

    #[test]
    fn test_progress_bar_render() {
        let progress = Progress::new()
            .progress(0.5)
            .width(10)
            .style(ProgressStyle::Block);

        let bar = progress.render_bar();
        assert_eq!(bar.chars().count(), 10);
        assert!(bar.contains('█'));
        assert!(bar.contains('░'));
    }

    #[test]
    fn test_progress_ascii_style() {
        let progress = Progress::new()
            .progress(0.5)
            .width(12) // 10 + 2 for brackets
            .style(ProgressStyle::Ascii);

        let bar = progress.render_bar();
        assert!(bar.starts_with('['));
        assert!(bar.ends_with(']'));
        assert!(bar.contains('='));
        assert!(bar.contains('-'));
    }

    #[test]
    fn test_progress_to_node() {
        let progress = Progress::new()
            .progress(0.5)
            .width(20)
            .show_percentage(true);

        let node: Node = progress.into();
        assert!(matches!(node, Node::Box(_)));
    }

    #[test]
    fn test_progress_styles() {
        // Verify all styles render without panic
        for style in [
            ProgressStyle::Block,
            ProgressStyle::Ascii,
            ProgressStyle::Unicode,
            ProgressStyle::Thin,
            ProgressStyle::Braille,
        ] {
            let progress = Progress::new().progress(0.5).style(style);
            let _ = progress.render_bar();
        }
    }

    #[test]
    fn test_progress_adaptive_tier0() {
        let progress = Progress::new().progress(0.75);

        let node = progress.render_for_tier(RenderTier::Tier0Fallback);
        // Should render as text node with percentage
        assert!(matches!(node, Node::Text(_)));
    }

    #[test]
    fn test_progress_adaptive_tier0_with_value() {
        let progress = Progress::new().value(75, 100);

        let node = progress.render_for_tier(RenderTier::Tier0Fallback);
        // Should render as text node with value
        assert!(matches!(node, Node::Text(_)));
    }

    #[test]
    fn test_progress_adaptive_tier1() {
        let progress = Progress::new().progress(0.5).width(20);

        let node = progress.render_for_tier(RenderTier::Tier1Ansi);
        // Should render as box with ASCII bar
        assert!(matches!(node, Node::Box(_)));
    }

    #[test]
    fn test_progress_adaptive_tier2() {
        let progress = Progress::new().progress(0.5);

        let node = progress.render_for_tier(RenderTier::Tier2Retained);
        // Should render same as default (Unicode with colors)
        assert!(matches!(node, Node::Box(_)));
    }

    #[test]
    fn test_progress_adaptive_all_tiers() {
        let progress = Progress::new().progress(0.75);

        // Should render without panic at all tiers
        for tier in [
            RenderTier::Tier0Fallback,
            RenderTier::Tier1Ansi,
            RenderTier::Tier2Retained,
            RenderTier::Tier3Gpu,
        ] {
            let _node = progress.render_for_tier(tier);
        }
    }

    #[test]
    fn test_progress_tier_features() {
        let progress = Progress::new().progress(0.5);
        let features = progress.tier_features();

        assert_eq!(features.name, Some("Progress"));
        assert!(features.tier0_description.is_some());
        assert!(features.tier1_description.is_some());
        assert!(features.tier2_description.is_some());
        assert!(features.tier3_description.is_some());
        assert!(features.gpu_enhanced);
    }

    #[test]
    fn test_progress_supports_all_tiers() {
        let progress = Progress::new();

        // Progress should work at all tiers
        assert!(progress.supports_tier(RenderTier::Tier0Fallback));
        assert!(progress.supports_tier(RenderTier::Tier1Ansi));
        assert!(progress.supports_tier(RenderTier::Tier2Retained));
        assert!(progress.supports_tier(RenderTier::Tier3Gpu));
    }
}
