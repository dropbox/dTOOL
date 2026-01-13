//! Spinner component - animated loading indicator.

use crate::node::{Node, TextNode};
use crate::style::Color;

/// Spinner animation style.
#[derive(Debug, Clone, Copy, Default)]
pub enum SpinnerStyle {
    /// Classic dots: ⣾ ⣽ ⣻ ⢿ ⡿ ⣟ ⣯ ⣷
    #[default]
    Dots,
    /// Line: - \ | /
    Line,
    /// Simple: | / - \
    Simple,
    /// Circle: ◐ ◓ ◑ ◒
    Circle,
    /// Arrow: ← ↖ ↑ ↗ → ↘ ↓ ↙
    Arrow,
    /// Bouncing bar: [=   ] [ =  ] [  = ] [   =]
    BouncingBar,
}

impl SpinnerStyle {
    /// Get frames for this spinner style.
    pub fn frames(&self) -> &'static [&'static str] {
        match self {
            SpinnerStyle::Dots => &["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"],
            SpinnerStyle::Line => &["-", "\\", "|", "/"],
            SpinnerStyle::Simple => &["|", "/", "-", "\\"],
            SpinnerStyle::Circle => &["◐", "◓", "◑", "◒"],
            SpinnerStyle::Arrow => &["←", "↖", "↑", "↗", "→", "↘", "↓", "↙"],
            SpinnerStyle::BouncingBar => {
                &["[=   ]", "[ =  ]", "[  = ]", "[   =]", "[  = ]", "[ =  ]"]
            }
        }
    }

    /// Get the frame at a given index (wraps around).
    pub fn frame(&self, index: usize) -> &'static str {
        let frames = self.frames();
        frames[index % frames.len()]
    }
}

/// Animated spinner component.
#[derive(Debug, Clone)]
pub struct Spinner {
    /// Animation style.
    style: SpinnerStyle,
    /// Current frame index.
    frame: usize,
    /// Color.
    color: Option<Color>,
    /// Optional label after spinner.
    label: Option<String>,
}

impl Spinner {
    /// Create a new spinner.
    pub fn new() -> Self {
        Self {
            style: SpinnerStyle::default(),
            frame: 0,
            color: None,
            label: None,
        }
    }

    /// Set spinner style.
    pub fn style(mut self, style: SpinnerStyle) -> Self {
        self.style = style;
        self
    }

    /// Set current frame.
    pub fn frame(mut self, frame: usize) -> Self {
        self.frame = frame;
        self
    }

    /// Set color.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Set label.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Advance to next frame.
    pub fn tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);
    }

    /// Get current display text.
    pub fn text(&self) -> String {
        let frame = self.style.frame(self.frame);
        match &self.label {
            Some(label) => format!("{} {}", frame, label),
            None => frame.to_string(),
        }
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Spinner> for Node {
    fn from(spinner: Spinner) -> Self {
        let mut text = TextNode::new(spinner.text());
        if let Some(color) = spinner.color {
            text = text.color(color);
        }
        text.into()
    }
}
