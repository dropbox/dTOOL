//! Adaptive rendering support for graceful degradation.
//!
//! This module provides traits and utilities for components to adapt their
//! rendering based on terminal capabilities. Components can implement
//! [`AdaptiveComponent`] to provide tier-specific rendering strategies.
//!
//! # Tier System
//!
//! inky uses a four-tier capability system:
//!
//! | Tier | Name | Capabilities |
//! |------|------|--------------|
//! | 3 | GPU | Full GPU acceleration, 120 FPS, shaders |
//! | 2 | Retained | True color, synchronized output, mouse |
//! | 1 | ANSI | 256 colors, basic Unicode |
//! | 0 | Fallback | Text only, no colors, safe for CI |
//!
//! # Example
//!
//! ```ignore
//! use inky::components::adaptive::{AdaptiveComponent, TierFeatures};
//! use inky::terminal::RenderTier;
//! use inky::node::Node;
//!
//! struct MyWidget {
//!     data: Vec<f64>,
//! }
//!
//! impl AdaptiveComponent for MyWidget {
//!     fn render_for_tier(&self, tier: RenderTier) -> Node {
//!         match tier {
//!             RenderTier::Tier3Gpu => self.render_gpu(),
//!             RenderTier::Tier2Retained => self.render_unicode(),
//!             RenderTier::Tier1Ansi => self.render_ascii(),
//!             RenderTier::Tier0Fallback => self.render_text(),
//!         }
//!     }
//!
//!     fn tier_features(&self) -> TierFeatures {
//!         TierFeatures {
//!             tier0_description: "Shows numeric summary only",
//!             tier1_description: "ASCII chart with markers",
//!             tier2_description: "Unicode braille chart with colors",
//!             tier3_description: "GPU-accelerated smooth rendering",
//!             ..Default::default()
//!         }
//!     }
//! }
//! ```

use crate::node::{BoxNode, Node, TextNode};
use crate::style::{Color, FlexDirection};
use crate::terminal::RenderTier;

/// Trait for components that adapt their rendering based on terminal capabilities.
///
/// Implement this trait to provide tier-specific rendering strategies. This allows
/// your component to work on any terminal while taking advantage of advanced
/// features when available.
///
/// # Required Methods
///
/// - [`render_for_tier`](AdaptiveComponent::render_for_tier) - Render the component
///   for a specific capability tier
///
/// # Optional Methods
///
/// - [`tier_features`](AdaptiveComponent::tier_features) - Describe what features
///   are available at each tier (for documentation and upgrade prompts)
/// - [`minimum_tier`](AdaptiveComponent::minimum_tier) - Specify the minimum
///   required tier for this component to function meaningfully
pub trait AdaptiveComponent {
    /// Render the component for a specific capability tier.
    ///
    /// This method should return an appropriate representation for the given tier.
    /// Lower tiers should provide gracefully degraded output that still conveys
    /// the essential information.
    fn render_for_tier(&self, tier: RenderTier) -> Node;

    /// Describe the features available at each tier.
    ///
    /// This information is used for documentation and upgrade prompts.
    fn tier_features(&self) -> TierFeatures {
        TierFeatures::default()
    }

    /// Get the minimum tier required for this component to be useful.
    ///
    /// Returns `None` if the component works at all tiers, or `Some(tier)` if
    /// a minimum tier is required for meaningful output.
    fn minimum_tier(&self) -> Option<RenderTier> {
        None
    }

    /// Check if this component can render at the given tier.
    fn supports_tier(&self, tier: RenderTier) -> bool {
        self.minimum_tier().map_or(true, |min| tier >= min)
    }

    /// Get a warning message if the component is degraded at this tier.
    fn degradation_warning(&self, tier: RenderTier) -> Option<String> {
        let features = self.tier_features();
        if tier < RenderTier::Tier2Retained && features.tier2_description.is_some() {
            Some(format!(
                "Degraded: {}",
                features
                    .description_for_tier(tier)
                    .unwrap_or("limited features")
            ))
        } else {
            None
        }
    }
}

/// Description of features available at each rendering tier.
///
/// Use this to document what capabilities are available and help users
/// understand what they'll see at different capability levels.
#[derive(Debug, Clone, Default)]
pub struct TierFeatures {
    /// Human-readable component name.
    pub name: Option<&'static str>,

    /// Description of Tier 0 (fallback) rendering.
    pub tier0_description: Option<&'static str>,

    /// Description of Tier 1 (ANSI) rendering.
    pub tier1_description: Option<&'static str>,

    /// Description of Tier 2 (retained) rendering.
    pub tier2_description: Option<&'static str>,

    /// Description of Tier 3 (GPU) rendering.
    pub tier3_description: Option<&'static str>,

    /// Whether this component benefits from GPU acceleration.
    pub gpu_enhanced: bool,
}

impl TierFeatures {
    /// Create tier features with a component name.
    pub fn new(name: &'static str) -> Self {
        Self {
            name: Some(name),
            ..Default::default()
        }
    }

    /// Set the Tier 0 (fallback) description.
    pub fn tier0(mut self, description: &'static str) -> Self {
        self.tier0_description = Some(description);
        self
    }

    /// Set the Tier 1 (ANSI) description.
    pub fn tier1(mut self, description: &'static str) -> Self {
        self.tier1_description = Some(description);
        self
    }

    /// Set the Tier 2 (retained) description.
    pub fn tier2(mut self, description: &'static str) -> Self {
        self.tier2_description = Some(description);
        self
    }

    /// Set the Tier 3 (GPU) description.
    pub fn tier3(mut self, description: &'static str) -> Self {
        self.tier3_description = Some(description);
        self.gpu_enhanced = true;
        self
    }

    /// Mark this component as GPU-enhanced.
    pub fn gpu(mut self) -> Self {
        self.gpu_enhanced = true;
        self
    }

    /// Get the description for a specific tier.
    pub fn description_for_tier(&self, tier: RenderTier) -> Option<&'static str> {
        match tier {
            RenderTier::Tier0Fallback => self.tier0_description,
            RenderTier::Tier1Ansi => self.tier1_description,
            RenderTier::Tier2Retained => self.tier2_description,
            RenderTier::Tier3Gpu => self.tier3_description,
        }
    }
}

/// Fallback text representation for Tier 0 rendering.
///
/// Use this helper to create consistent text-only representations.
///
/// # Example
///
/// ```ignore
/// let fallback = Tier0Fallback::new("Progress")
///     .stat("value", "75%")
///     .stat("elapsed", "2m 30s");
/// ```
#[derive(Debug, Clone)]
pub struct Tier0Fallback {
    label: String,
    stats: Vec<(String, String)>,
}

impl Tier0Fallback {
    /// Create a new Tier 0 fallback with a label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            stats: Vec::new(),
        }
    }

    /// Add a statistic to display.
    pub fn stat(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.stats.push((name.into(), value.into()));
        self
    }

    /// Render to a plain text string.
    pub fn to_text(&self) -> String {
        use std::fmt::Write;
        let mut result = format!("[{}]", self.label);
        for (name, value) in &self.stats {
            let _ = write!(result, " {}={}", name, value);
        }
        result
    }
}

impl From<Tier0Fallback> for Node {
    fn from(fallback: Tier0Fallback) -> Self {
        TextNode::new(fallback.to_text()).into()
    }
}

/// ASCII representation for Tier 1 rendering.
///
/// Provides helpers for creating ASCII-art visualizations.
#[derive(Debug, Clone, Default)]
pub struct AsciiRenderer {
    width: usize,
    height: usize,
    chars: Vec<char>,
}

impl AsciiRenderer {
    /// Create a new ASCII renderer with given dimensions.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            chars: vec![' '; width * height],
        }
    }

    /// Set a character at the given position.
    pub fn set(&mut self, x: usize, y: usize, ch: char) {
        if x < self.width && y < self.height {
            self.chars[y * self.width + x] = ch;
        }
    }

    /// Draw a horizontal line.
    pub fn hline(&mut self, y: usize, x_start: usize, x_end: usize, ch: char) {
        for x in x_start..=x_end.min(self.width.saturating_sub(1)) {
            self.set(x, y, ch);
        }
    }

    /// Draw a vertical line.
    pub fn vline(&mut self, x: usize, y_start: usize, y_end: usize, ch: char) {
        for y in y_start..=y_end.min(self.height.saturating_sub(1)) {
            self.set(x, y, ch);
        }
    }

    /// Draw a box outline.
    pub fn outline(&mut self) {
        if self.width < 2 || self.height < 2 {
            return;
        }
        // Corners
        self.set(0, 0, '+');
        self.set(self.width - 1, 0, '+');
        self.set(0, self.height - 1, '+');
        self.set(self.width - 1, self.height - 1, '+');
        // Edges
        self.hline(0, 1, self.width - 2, '-');
        self.hline(self.height - 1, 1, self.width - 2, '-');
        self.vline(0, 1, self.height - 2, '|');
        self.vline(self.width - 1, 1, self.height - 2, '|');
    }

    /// Render to lines of text.
    pub fn to_lines(&self) -> Vec<String> {
        (0..self.height)
            .map(|y| {
                self.chars[y * self.width..(y + 1) * self.width]
                    .iter()
                    .collect()
            })
            .collect()
    }

    /// Convert to a Node.
    pub fn to_node(&self) -> Node {
        let lines = self.to_lines();
        let mut container = BoxNode::new().flex_direction(FlexDirection::Column);
        for line in lines {
            container = container.child(TextNode::new(line));
        }
        container.into()
    }
}

/// Helper to create consistent degradation notices.
///
/// Shows the user what they're missing and optionally suggests an upgrade.
#[derive(Debug, Clone)]
pub struct DegradationNotice {
    component_name: String,
    current_tier: RenderTier,
    message: Option<String>,
    show_upgrade_hint: bool,
}

impl DegradationNotice {
    /// Create a new degradation notice.
    pub fn new(component_name: impl Into<String>, current_tier: RenderTier) -> Self {
        Self {
            component_name: component_name.into(),
            current_tier,
            message: None,
            show_upgrade_hint: false,
        }
    }

    /// Set a custom message.
    pub fn message(mut self, msg: impl Into<String>) -> Self {
        self.message = Some(msg.into());
        self
    }

    /// Show an upgrade hint.
    pub fn upgrade_hint(mut self) -> Self {
        self.show_upgrade_hint = true;
        self
    }

    /// Render to a node (dimmed text).
    pub fn to_node(&self) -> Node {
        let msg = self.message.clone().unwrap_or_else(|| {
            format!(
                "{} ({})",
                self.component_name,
                self.current_tier.description()
            )
        });

        let mut text = TextNode::new(msg).color(Color::BrightBlack);

        if self.show_upgrade_hint && self.current_tier < RenderTier::Tier2Retained {
            text = text.italic();
        }

        text.into()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_features_builder() {
        let features = TierFeatures::new("TestComponent")
            .tier0("Text only")
            .tier1("ASCII art")
            .tier2("Unicode with colors")
            .tier3("GPU shaders");

        assert_eq!(features.name, Some("TestComponent"));
        assert_eq!(features.tier0_description, Some("Text only"));
        assert_eq!(features.tier3_description, Some("GPU shaders"));
        assert!(features.gpu_enhanced);
    }

    #[test]
    fn test_tier0_fallback() {
        let fallback = Tier0Fallback::new("Progress")
            .stat("value", "75%")
            .stat("elapsed", "2m");

        let text = fallback.to_text();
        assert!(text.contains("[Progress]"));
        assert!(text.contains("value=75%"));
        assert!(text.contains("elapsed=2m"));
    }

    #[test]
    fn test_ascii_renderer() {
        let mut renderer = AsciiRenderer::new(10, 5);
        renderer.outline();
        renderer.set(5, 2, '*');

        let lines = renderer.to_lines();
        assert_eq!(lines.len(), 5);
        assert!(lines[0].starts_with('+'));
        assert!(lines[2].contains('*'));
    }

    #[test]
    fn test_degradation_notice() {
        let notice = DegradationNotice::new("Heatmap", RenderTier::Tier0Fallback)
            .message("Limited to text summary")
            .upgrade_hint();

        let node = notice.to_node();
        assert!(matches!(node, Node::Text(_)));
    }

    struct TestComponent {
        value: f32,
    }

    impl AdaptiveComponent for TestComponent {
        fn render_for_tier(&self, tier: RenderTier) -> Node {
            match tier {
                RenderTier::Tier0Fallback => Tier0Fallback::new("Test")
                    .stat("value", format!("{:.0}%", self.value * 100.0))
                    .into(),
                _ => TextNode::new(format!("Value: {:.1}%", self.value * 100.0)).into(),
            }
        }

        fn tier_features(&self) -> TierFeatures {
            TierFeatures::new("TestComponent")
                .tier0("Numeric value only")
                .tier1("Formatted percentage")
        }

        fn minimum_tier(&self) -> Option<RenderTier> {
            None
        }
    }

    #[test]
    fn test_adaptive_component_trait() {
        let comp = TestComponent { value: 0.75 };

        // Should work at all tiers
        assert!(comp.supports_tier(RenderTier::Tier0Fallback));
        assert!(comp.supports_tier(RenderTier::Tier3Gpu));

        // Should render appropriately for each tier
        let _node0 = comp.render_for_tier(RenderTier::Tier0Fallback);
        let _node2 = comp.render_for_tier(RenderTier::Tier2Retained);
    }
}
