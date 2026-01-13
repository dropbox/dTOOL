//! Upgrade prompt component for suggesting terminal upgrades.
//!
//! This component displays a non-intrusive prompt when the current terminal
//! capabilities are limiting the user experience. It helps users discover
//! better terminals that support advanced features.
//!
//! # Example
//!
//! ```ignore
//! use inky::components::UpgradePrompt;
//! use inky::terminal::{Capabilities, RenderTier};
//!
//! let caps = Capabilities::detect();
//! if caps.tier() < RenderTier::Tier2Retained {
//!     let prompt = UpgradePrompt::new(caps.tier())
//!         .feature("true color")
//!         .feature("mouse support")
//!         .recommendation("iTerm2", "https://iterm2.com");
//! }
//! ```

use crate::node::{BoxNode, Node, TextNode};
use crate::style::{Color, FlexDirection};
use crate::terminal::RenderTier;

/// A prompt suggesting terminal upgrades for better features.
///
/// This component is designed to be:
/// - Non-intrusive: small, dismissible, and doesn't block interaction
/// - Helpful: explains what features are limited
/// - Actionable: provides specific recommendations
#[derive(Debug, Clone)]
pub struct UpgradePrompt {
    /// Current rendering tier.
    current_tier: RenderTier,
    /// Target tier to recommend.
    target_tier: RenderTier,
    /// List of missing features.
    missing_features: Vec<String>,
    /// Recommended terminal and URL.
    recommendation: Option<(String, String)>,
    /// Whether to show performance comparison.
    show_performance: bool,
    /// Whether the prompt can be dismissed.
    dismissible: bool,
    /// Custom message.
    custom_message: Option<String>,
    /// Border style (true for Unicode box, false for ASCII).
    unicode_border: bool,
    /// Width of the prompt.
    width: u16,
}

impl UpgradePrompt {
    /// Create a new upgrade prompt for the current tier.
    pub fn new(current_tier: RenderTier) -> Self {
        // Default target is one tier up, or Tier2 minimum
        let target_tier = match current_tier {
            RenderTier::Tier0Fallback => RenderTier::Tier1Ansi,
            RenderTier::Tier1Ansi => RenderTier::Tier2Retained,
            _ => RenderTier::Tier3Gpu,
        };

        Self {
            current_tier,
            target_tier,
            missing_features: Vec::new(),
            recommendation: None,
            show_performance: true,
            dismissible: true,
            custom_message: None,
            unicode_border: current_tier >= RenderTier::Tier1Ansi,
            width: 60,
        }
    }

    /// Set the target tier to recommend.
    pub fn target(mut self, tier: RenderTier) -> Self {
        self.target_tier = tier;
        self
    }

    /// Add a missing feature to highlight.
    pub fn feature(mut self, feature: impl Into<String>) -> Self {
        self.missing_features.push(feature.into());
        self
    }

    /// Add multiple missing features.
    pub fn features(mut self, features: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.missing_features
            .extend(features.into_iter().map(Into::into));
        self
    }

    /// Set a recommended terminal with download URL.
    pub fn recommendation(mut self, name: impl Into<String>, url: impl Into<String>) -> Self {
        self.recommendation = Some((name.into(), url.into()));
        self
    }

    /// Show or hide performance comparison.
    pub fn show_performance(mut self, show: bool) -> Self {
        self.show_performance = show;
        self
    }

    /// Set whether the prompt is dismissible.
    pub fn dismissible(mut self, dismissible: bool) -> Self {
        self.dismissible = dismissible;
        self
    }

    /// Set a custom message to display.
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.custom_message = Some(message.into());
        self
    }

    /// Set the width of the prompt.
    pub fn width(mut self, width: u16) -> Self {
        self.width = width;
        self
    }

    /// Get the current tier.
    pub fn current_tier(&self) -> RenderTier {
        self.current_tier
    }

    /// Get the target tier.
    pub fn target_tier(&self) -> RenderTier {
        self.target_tier
    }

    /// Check if an upgrade is recommended.
    pub fn should_show(&self) -> bool {
        self.current_tier < self.target_tier
    }

    /// Render the prompt header.
    fn render_header(&self) -> Node {
        let icon = if self.unicode_border { "⚡" } else { "*" };
        let text = format!(
            "{} This visualization runs faster in {}",
            icon,
            self.target_tier.name()
        );
        TextNode::new(text).bold().into()
    }

    /// Render the tier comparison.
    fn render_comparison(&self) -> Node {
        let mut container = BoxNode::new()
            .flex_direction(FlexDirection::Column)
            .gap(0.0);

        // Current tier
        let current = format!(
            "Current: {} ({} FPS, {})",
            self.current_tier.name(),
            self.current_tier.target_fps(),
            self.current_tier.description()
        );
        container = container.child(TextNode::new(current).color(Color::BrightBlack));

        // Target tier
        let target = format!(
            "Recommended: {} ({} FPS, {})",
            self.target_tier.name(),
            self.target_tier.target_fps(),
            self.target_tier.description()
        );
        container = container.child(TextNode::new(target).color(Color::Green));

        container.into()
    }

    /// Render missing features list.
    fn render_features(&self) -> Option<Node> {
        if self.missing_features.is_empty() {
            return None;
        }

        let mut container = BoxNode::new()
            .flex_direction(FlexDirection::Column)
            .gap(0.0);

        container = container.child(TextNode::new("Missing features:").color(Color::Yellow));

        for feature in &self.missing_features {
            let bullet = if self.unicode_border {
                "  • "
            } else {
                "  - "
            };
            container = container.child(TextNode::new(format!("{}{}", bullet, feature)));
        }

        Some(container.into())
    }

    /// Render the recommendation.
    fn render_recommendation(&self) -> Option<Node> {
        let (name, url) = self.recommendation.as_ref()?;

        let mut container = BoxNode::new().flex_direction(FlexDirection::Row).gap(1.0);

        container =
            container.child(TextNode::new(format!("[Download {}]", name)).color(Color::Cyan));
        container = container.child(TextNode::new(url.clone()).color(Color::BrightBlack));

        Some(container.into())
    }

    /// Render dismissal hint.
    fn render_dismiss_hint(&self) -> Option<Node> {
        if !self.dismissible {
            return None;
        }

        Some(
            TextNode::new("[Dismiss]  [Don't show again]")
                .color(Color::BrightBlack)
                .into(),
        )
    }

    /// Build the border top.
    fn border_top(&self) -> String {
        if self.unicode_border {
            format!("┌{}┐", "─".repeat((self.width as usize).saturating_sub(2)))
        } else {
            format!("+{}+", "-".repeat((self.width as usize).saturating_sub(2)))
        }
    }

    /// Build the border bottom.
    fn border_bottom(&self) -> String {
        if self.unicode_border {
            format!("└{}┘", "─".repeat((self.width as usize).saturating_sub(2)))
        } else {
            format!("+{}+", "-".repeat((self.width as usize).saturating_sub(2)))
        }
    }
}

impl Default for UpgradePrompt {
    fn default() -> Self {
        Self::new(RenderTier::Tier1Ansi)
    }
}

impl From<UpgradePrompt> for Node {
    fn from(prompt: UpgradePrompt) -> Self {
        if !prompt.should_show() {
            // Return empty box if no upgrade needed
            return BoxNode::new().into();
        }

        let mut container = BoxNode::new()
            .flex_direction(FlexDirection::Column)
            .gap(1.0);

        // Border top
        container = container.child(TextNode::new(prompt.border_top()).color(Color::BrightBlack));

        // Content container
        let mut content = BoxNode::new()
            .flex_direction(FlexDirection::Column)
            .gap(1.0)
            .padding_xy(2.0, 0.0);

        // Custom message or header
        if let Some(ref msg) = prompt.custom_message {
            content = content.child(TextNode::new(msg.clone()));
        } else {
            content = content.child(prompt.render_header());
        }

        // Empty line
        content = content.child(TextNode::new(""));

        // Performance comparison
        if prompt.show_performance {
            content = content.child(prompt.render_comparison());
        }

        // Missing features
        if let Some(features_node) = prompt.render_features() {
            content = content.child(TextNode::new(""));
            content = content.child(features_node);
        }

        // Recommendation
        if let Some(rec_node) = prompt.render_recommendation() {
            content = content.child(TextNode::new(""));
            content = content.child(rec_node);
        }

        // Dismissal hint
        if let Some(dismiss_node) = prompt.render_dismiss_hint() {
            content = content.child(TextNode::new(""));
            content = content.child(dismiss_node);
        }

        container = container.child(content);

        // Border bottom
        container =
            container.child(TextNode::new(prompt.border_bottom()).color(Color::BrightBlack));

        container.into()
    }
}

/// Builder for common upgrade prompt configurations.
pub struct UpgradePromptPresets;

impl UpgradePromptPresets {
    /// Create a prompt for suggesting iTerm2 on macOS.
    pub fn iterm2(current_tier: RenderTier) -> UpgradePrompt {
        UpgradePrompt::new(current_tier)
            .target(RenderTier::Tier2Retained)
            .feature("true color (24-bit RGB)")
            .feature("synchronized output (no tearing)")
            .feature("mouse support")
            .feature("Sixel graphics")
            .recommendation("iTerm2", "https://iterm2.com")
    }

    /// Create a prompt for suggesting Kitty.
    pub fn kitty(current_tier: RenderTier) -> UpgradePrompt {
        UpgradePrompt::new(current_tier)
            .target(RenderTier::Tier2Retained)
            .feature("true color (24-bit RGB)")
            .feature("Kitty keyboard protocol")
            .feature("Kitty graphics protocol")
            .feature("ligature support")
            .recommendation("Kitty", "https://sw.kovidgoyal.net/kitty/")
    }

    /// Create a prompt for suggesting WezTerm.
    pub fn wezterm(current_tier: RenderTier) -> UpgradePrompt {
        UpgradePrompt::new(current_tier)
            .target(RenderTier::Tier2Retained)
            .feature("true color (24-bit RGB)")
            .feature("GPU acceleration")
            .feature("multiplexer built-in")
            .recommendation("WezTerm", "https://wezfurlong.org/wezterm/")
    }

    /// Create a prompt for suggesting Alacritty.
    pub fn alacritty(current_tier: RenderTier) -> UpgradePrompt {
        UpgradePrompt::new(current_tier)
            .target(RenderTier::Tier2Retained)
            .feature("true color (24-bit RGB)")
            .feature("GPU acceleration (OpenGL)")
            .feature("minimal latency")
            .recommendation("Alacritty", "https://alacritty.org")
    }

    /// Create a prompt for suggesting dterm/dashterm (Tier 3 GPU).
    pub fn dterm(current_tier: RenderTier) -> UpgradePrompt {
        UpgradePrompt::new(current_tier)
            .target(RenderTier::Tier3Gpu)
            .feature("120 FPS rendering")
            .feature("<1ms input latency")
            .feature("GPU shaders for visualization")
            .feature("zero-copy buffer access")
            .recommendation("dashterm2", "https://github.com/dropbox/dterm")
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_upgrade_prompt_new() {
        let prompt = UpgradePrompt::new(RenderTier::Tier1Ansi);
        assert_eq!(prompt.current_tier(), RenderTier::Tier1Ansi);
        assert_eq!(prompt.target_tier(), RenderTier::Tier2Retained);
        assert!(prompt.should_show());
    }

    #[test]
    fn test_upgrade_prompt_no_upgrade_needed() {
        let prompt = UpgradePrompt::new(RenderTier::Tier3Gpu);
        assert!(!prompt.should_show());
    }

    #[test]
    fn test_upgrade_prompt_features() {
        let prompt = UpgradePrompt::new(RenderTier::Tier0Fallback)
            .feature("colors")
            .feature("unicode");

        assert_eq!(prompt.missing_features.len(), 2);
    }

    #[test]
    fn test_upgrade_prompt_recommendation() {
        let prompt = UpgradePrompt::new(RenderTier::Tier1Ansi)
            .recommendation("TestTerm", "https://test.com");

        assert!(prompt.recommendation.is_some());
        let (name, url) = prompt.recommendation.unwrap();
        assert_eq!(name, "TestTerm");
        assert_eq!(url, "https://test.com");
    }

    #[test]
    fn test_upgrade_prompt_to_node() {
        let prompt = UpgradePrompt::new(RenderTier::Tier1Ansi)
            .feature("true color")
            .recommendation("TestTerm", "https://test.com");

        let node: Node = prompt.into();
        assert!(matches!(node, Node::Box(_)));
    }

    #[test]
    fn test_upgrade_prompt_presets() {
        let iterm = UpgradePromptPresets::iterm2(RenderTier::Tier1Ansi);
        assert!(iterm.should_show());
        assert!(!iterm.missing_features.is_empty());

        let kitty = UpgradePromptPresets::kitty(RenderTier::Tier0Fallback);
        assert!(kitty.should_show());

        let dterm = UpgradePromptPresets::dterm(RenderTier::Tier2Retained);
        assert_eq!(dterm.target_tier(), RenderTier::Tier3Gpu);
    }

    #[test]
    fn test_border_styles() {
        // Unicode border for Tier 1+
        let prompt_unicode = UpgradePrompt::new(RenderTier::Tier1Ansi);
        assert!(prompt_unicode.unicode_border);
        assert!(prompt_unicode.border_top().contains('─'));

        // ASCII border for Tier 0
        let prompt_ascii = UpgradePrompt::new(RenderTier::Tier0Fallback);
        assert!(!prompt_ascii.unicode_border);
        assert!(prompt_ascii.border_top().contains('-'));
    }

    #[test]
    fn test_empty_when_no_upgrade() {
        let prompt = UpgradePrompt::new(RenderTier::Tier3Gpu);
        let node: Node = prompt.into();
        // Should be an empty box
        if let Node::Box(box_node) = node {
            assert!(box_node.children.is_empty());
        } else {
            panic!("Expected Box node");
        }
    }
}
