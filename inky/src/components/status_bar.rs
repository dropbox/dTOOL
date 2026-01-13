//! Status bar components for displaying operational status.
//!
//! Provides status bar components that display application state with animated
//! spinners, color-coded indicators, and operational metrics.
//!
//! # Components
//!
//! - [`StatusBar`] - Simple state-based status (Idle/Thinking/Executing/Error)
//! - [`TokenStatusBar`] - Rich status bar with tokens, cost, context, and more
//!
//! # Simple Status Bar
//!
//! ```
//! use inky::components::{StatusBar, StatusState};
//!
//! let status = StatusBar::new()
//!     .state(StatusState::Thinking)
//!     .message("Processing request...");
//! ```
//!
//! # Rich Token Status Bar
//!
//! For Claude Code-style status displays with metrics:
//!
//! ```
//! use inky::components::{TokenStatusBar, StatusState, DataDirection};
//! use std::time::Duration;
//!
//! let status = TokenStatusBar::new()
//!     .state(StatusState::Thinking)
//!     .tokens(1523, 4891)
//!     .cost(0.0523)
//!     .context_percent(65.0)
//!     .elapsed(Duration::from_secs(45))
//!     .direction(DataDirection::Receiving)
//!     .cwd("/Users/user/project");
//! ```
//!
//! Output: `⣾ Thinking... | 1.5k/4.9k tokens | $0.05 | 65% ⚠ | 0:45 | ↓ | ~/project`
//!
//! # Status States
//!
//! - **Idle** - Ready state (green indicator)
//! - **Thinking** - Processing state with spinner (yellow)
//! - **Executing** - Running a command with spinner (blue)
//! - **Error** - Error state (red indicator)
//!
//! # Animation
//!
//! For `Thinking` and `Executing` states, call `tick()` periodically to advance
//! the spinner animation:
//!
//! ```
//! use inky::components::{StatusBar, StatusState};
//!
//! let mut status = StatusBar::new().state(StatusState::Thinking);
//!
//! // In your update loop:
//! status.tick();
//! let node = status.to_node();
//! ```
//!
//! # Adaptive Rendering
//!
//! Both components implement [`AdaptiveComponent`](crate::components::adaptive::AdaptiveComponent)
//! for graceful degradation across terminal capabilities.

use std::time::Duration;

use crate::components::adaptive::{AdaptiveComponent, TierFeatures};
use crate::components::SpinnerStyle;
use crate::node::{BoxNode, Node, TextNode};
use crate::style::{Color, FlexDirection};
use crate::terminal::RenderTier;

/// Operational status state.
///
/// Represents the current state of an operation or application.
///
/// # Example
///
/// ```
/// use inky::components::StatusState;
///
/// let state = StatusState::Thinking;
/// assert!(state.is_active());
///
/// let idle = StatusState::Idle;
/// assert!(!idle.is_active());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatusState {
    /// Ready/idle state. No active operation.
    #[default]
    Idle,
    /// Thinking/processing state. Operation is being planned or analyzed.
    Thinking,
    /// Executing state. A command or action is being run.
    Executing,
    /// Error state. Something went wrong.
    Error,
}

impl StatusState {
    /// Returns true if this is an active state (Thinking or Executing).
    ///
    /// Active states show an animated spinner.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::StatusState;
    ///
    /// assert!(!StatusState::Idle.is_active());
    /// assert!(StatusState::Thinking.is_active());
    /// assert!(StatusState::Executing.is_active());
    /// assert!(!StatusState::Error.is_active());
    /// ```
    pub fn is_active(&self) -> bool {
        matches!(self, StatusState::Thinking | StatusState::Executing)
    }

    /// Get the color associated with this state.
    ///
    /// - Idle: Green
    /// - Thinking: Yellow
    /// - Executing: Blue
    /// - Error: Red
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::StatusState;
    /// use inky::style::Color;
    ///
    /// assert_eq!(StatusState::Idle.color(), Color::Green);
    /// assert_eq!(StatusState::Error.color(), Color::Red);
    /// ```
    pub fn color(&self) -> Color {
        match self {
            StatusState::Idle => Color::Green,
            StatusState::Thinking => Color::Yellow,
            StatusState::Executing => Color::Blue,
            StatusState::Error => Color::Red,
        }
    }

    /// Get the default label for this state.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::StatusState;
    ///
    /// assert_eq!(StatusState::Idle.label(), "Ready");
    /// assert_eq!(StatusState::Thinking.label(), "Thinking");
    /// ```
    pub fn label(&self) -> &'static str {
        match self {
            StatusState::Idle => "Ready",
            StatusState::Thinking => "Thinking",
            StatusState::Executing => "Executing",
            StatusState::Error => "Error",
        }
    }

    /// Get the status indicator symbol for this state.
    ///
    /// Returns a static symbol for non-active states.
    /// Active states use the spinner instead.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::StatusState;
    ///
    /// assert_eq!(StatusState::Idle.indicator(), "●");
    /// assert_eq!(StatusState::Error.indicator(), "✗");
    /// ```
    pub fn indicator(&self) -> &'static str {
        match self {
            StatusState::Idle => "●",
            StatusState::Thinking | StatusState::Executing => "●", // Will be replaced by spinner
            StatusState::Error => "✗",
        }
    }
}

/// Data flow direction indicator.
///
/// Used to show whether data is being sent to or received from the API.
///
/// # Example
///
/// ```
/// use inky::components::DataDirection;
///
/// let dir = DataDirection::Receiving;
/// assert_eq!(dir.symbol(), "↓");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DataDirection {
    /// No active data transfer.
    #[default]
    Idle,
    /// Sending data (uploading to API).
    Sending,
    /// Receiving data (downloading from API).
    Receiving,
}

impl DataDirection {
    /// Get the Unicode symbol for this direction.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::DataDirection;
    ///
    /// assert_eq!(DataDirection::Idle.symbol(), "•");
    /// assert_eq!(DataDirection::Sending.symbol(), "↑");
    /// assert_eq!(DataDirection::Receiving.symbol(), "↓");
    /// ```
    pub fn symbol(&self) -> &'static str {
        match self {
            DataDirection::Idle => "•",
            DataDirection::Sending => "↑",
            DataDirection::Receiving => "↓",
        }
    }

    /// Get the ASCII symbol for this direction.
    pub fn ascii_symbol(&self) -> &'static str {
        match self {
            DataDirection::Idle => ".",
            DataDirection::Sending => "^",
            DataDirection::Receiving => "v",
        }
    }
}

/// Status bar component displaying operational status.
///
/// Renders a status bar with:
/// - A status indicator (static symbol or animated spinner)
/// - A status label (state name or custom message)
/// - Color-coded styling based on state
///
/// # Example
///
/// ```
/// use inky::components::{StatusBar, StatusState};
///
/// // Default idle status
/// let status = StatusBar::new();
///
/// // Thinking with spinner
/// let mut status = StatusBar::new()
///     .state(StatusState::Thinking)
///     .message("Analyzing code...");
///
/// // Advance animation
/// status.tick();
///
/// // Convert to node for rendering
/// let node = status.to_node();
/// ```
///
/// # Spinner Styles
///
/// You can customize the spinner style for active states:
///
/// ```
/// use inky::components::{StatusBar, StatusState, SpinnerStyle};
///
/// let status = StatusBar::new()
///     .state(StatusState::Executing)
///     .spinner_style(SpinnerStyle::Circle);
/// ```
#[derive(Debug, Clone)]
pub struct StatusBar {
    /// Current operational state.
    state: StatusState,
    /// Optional custom message (overrides default state label).
    message: Option<String>,
    /// Spinner frame index for animation.
    spinner_frame: usize,
    /// Spinner style for active states.
    spinner_style: SpinnerStyle,
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusBar {
    /// Create a new status bar with default idle state.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::StatusBar;
    ///
    /// let status = StatusBar::new();
    /// ```
    pub fn new() -> Self {
        Self {
            state: StatusState::default(),
            message: None,
            spinner_frame: 0,
            spinner_style: SpinnerStyle::Dots,
        }
    }

    /// Set the status state.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::{StatusBar, StatusState};
    ///
    /// let status = StatusBar::new()
    ///     .state(StatusState::Executing);
    /// ```
    pub fn state(mut self, state: StatusState) -> Self {
        self.state = state;
        self
    }

    /// Set a custom message.
    ///
    /// This overrides the default state label.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::{StatusBar, StatusState};
    ///
    /// let status = StatusBar::new()
    ///     .state(StatusState::Thinking)
    ///     .message("Processing your request...");
    /// ```
    pub fn message(mut self, msg: impl Into<String>) -> Self {
        self.message = Some(msg.into());
        self
    }

    /// Set the spinner style for active states.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::{StatusBar, StatusState, SpinnerStyle};
    ///
    /// let status = StatusBar::new()
    ///     .state(StatusState::Executing)
    ///     .spinner_style(SpinnerStyle::Circle);
    /// ```
    pub fn spinner_style(mut self, style: SpinnerStyle) -> Self {
        self.spinner_style = style;
        self
    }

    /// Advance the spinner animation by one frame.
    ///
    /// Call this periodically (e.g., every 80-100ms) to animate the spinner.
    /// Has no effect for non-active states.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::{StatusBar, StatusState};
    ///
    /// let mut status = StatusBar::new()
    ///     .state(StatusState::Thinking);
    ///
    /// // In your update loop:
    /// status.tick();
    /// ```
    pub fn tick(&mut self) {
        if self.state.is_active() {
            self.spinner_frame = self.spinner_frame.wrapping_add(1);
        }
    }

    /// Get the current status state.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::{StatusBar, StatusState};
    ///
    /// let status = StatusBar::new()
    ///     .state(StatusState::Error);
    ///
    /// assert_eq!(status.current_state(), StatusState::Error);
    /// ```
    pub fn current_state(&self) -> StatusState {
        self.state
    }

    /// Get the current message (custom or default).
    ///
    /// Returns the custom message if set, otherwise the default state label.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::{StatusBar, StatusState};
    ///
    /// let status = StatusBar::new()
    ///     .state(StatusState::Idle);
    /// assert_eq!(status.current_message(), "Ready");
    ///
    /// let status = StatusBar::new()
    ///     .message("Custom message");
    /// assert_eq!(status.current_message(), "Custom message");
    /// ```
    pub fn current_message(&self) -> &str {
        self.message.as_deref().unwrap_or(self.state.label())
    }

    /// Get the current spinner frame.
    ///
    /// Useful for testing animation advancement.
    pub fn spinner_frame(&self) -> usize {
        self.spinner_frame
    }

    /// Convert to a Node for rendering.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::{StatusBar, StatusState};
    ///
    /// let status = StatusBar::new()
    ///     .state(StatusState::Thinking)
    ///     .message("Working...");
    ///
    /// let node = status.to_node();
    /// ```
    pub fn to_node(&self) -> Node {
        let color = self.state.color();
        let message = self.current_message();

        // Status indicator (spinner for active, symbol for inactive)
        let indicator: Node = if self.state.is_active() {
            let spinner_text = self.spinner_style.frame(self.spinner_frame);
            TextNode::new(spinner_text).color(color).into()
        } else {
            TextNode::new(self.state.indicator()).color(color).into()
        };

        // Build directly with child() chain - avoids intermediate Vec allocation
        BoxNode::new()
            .flex_direction(FlexDirection::Row)
            .child(indicator)
            .child(TextNode::new(" "))
            .child(TextNode::new(message).color(color))
            .into()
    }

    /// Render Tier 0: Plain text without any formatting.
    fn render_tier0(&self) -> Node {
        let state_text = match self.state {
            StatusState::Idle => "IDLE",
            StatusState::Thinking => "THINKING",
            StatusState::Executing => "EXECUTING",
            StatusState::Error => "ERROR",
        };
        let message = self.current_message();
        TextNode::new(format!("[STATUS: {} - {}]", state_text, message)).into()
    }

    /// Render Tier 1: ASCII indicators without colors.
    fn render_tier1(&self) -> Node {
        // ASCII-safe indicators
        let indicator = match self.state {
            StatusState::Idle => "[+]",
            StatusState::Thinking => {
                // Simple ASCII spinner frames
                const FRAMES: &[&str] = &["[-]", "[\\]", "[|]", "[/]"];
                FRAMES[self.spinner_frame % FRAMES.len()]
            }
            StatusState::Executing => {
                const FRAMES: &[&str] = &["[.]", "[..]", "[...]", "[..]"];
                FRAMES[self.spinner_frame % FRAMES.len()]
            }
            StatusState::Error => "[!]",
        };

        let message = self.current_message();

        BoxNode::new()
            .flex_direction(FlexDirection::Row)
            .child(TextNode::new(indicator))
            .child(TextNode::new(" "))
            .child(TextNode::new(message))
            .into()
    }
}

impl AdaptiveComponent for StatusBar {
    fn render_for_tier(&self, tier: RenderTier) -> Node {
        match tier {
            RenderTier::Tier0Fallback => self.render_tier0(),
            RenderTier::Tier1Ansi => self.render_tier1(),
            RenderTier::Tier2Retained | RenderTier::Tier3Gpu => self.to_node(),
        }
    }

    fn tier_features(&self) -> TierFeatures {
        TierFeatures::new("StatusBar")
            .tier0("Plain text status label")
            .tier1("ASCII indicators with simple animation")
            .tier2("Unicode indicators with colors and spinners")
            .tier3("Full rendering with GPU acceleration")
    }

    fn minimum_tier(&self) -> Option<RenderTier> {
        None // Works at all tiers
    }
}

impl From<StatusBar> for Node {
    fn from(status_bar: StatusBar) -> Self {
        status_bar.to_node()
    }
}

// =============================================================================
// TokenStatusBar - Rich status bar for Claude Code
// =============================================================================

/// Rich status bar with token counts, cost, context, and operational metrics.
///
/// Designed for Claude Code-style status displays showing:
/// - Status state with spinner
/// - Token counts (input/output) with compact notation
/// - API cost with adaptive formatting
/// - Context window usage with warnings
/// - Elapsed time (shows after 30s by default)
/// - Data direction indicator
/// - Offline status
/// - Working directory
///
/// # Example
///
/// ```
/// use inky::components::{TokenStatusBar, StatusState, DataDirection};
/// use std::time::Duration;
///
/// let status = TokenStatusBar::new()
///     .state(StatusState::Thinking)
///     .tokens(1523, 4891)
///     .cost(0.0523)
///     .context_percent(65.0)
///     .elapsed(Duration::from_secs(45))
///     .direction(DataDirection::Receiving)
///     .cwd("/Users/user/project");
///
/// let node = status.to_node();
/// ```
///
/// # Visual Output
///
/// ```text
/// ⣾ Thinking... | 1.5k/4.9k tokens | $0.05 | 65% ⚠ | 0:45 | ↓ | ~/project
/// ```
#[derive(Debug, Clone)]
pub struct TokenStatusBar {
    /// Current operational state.
    state: StatusState,
    /// Optional custom message (overrides default state label).
    message: Option<String>,
    /// Spinner frame index for animation.
    spinner_frame: usize,
    /// Spinner style for active states.
    spinner_style: SpinnerStyle,
    /// Input token count.
    input_tokens: Option<u64>,
    /// Output token count.
    output_tokens: Option<u64>,
    /// API cost in dollars.
    cost: Option<f64>,
    /// Context window usage percentage (0-100).
    context_percent: Option<f64>,
    /// Elapsed time since operation started.
    elapsed: Option<Duration>,
    /// Minimum elapsed time before showing (default 30s).
    elapsed_threshold: Duration,
    /// Data direction indicator.
    direction: DataDirection,
    /// Whether the connection is offline.
    offline: bool,
    /// Current working directory.
    cwd: Option<String>,
    /// Maximum width for cwd display.
    cwd_max_width: usize,
}

impl Default for TokenStatusBar {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenStatusBar {
    /// Create a new token status bar with default idle state.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::TokenStatusBar;
    ///
    /// let status = TokenStatusBar::new();
    /// ```
    pub fn new() -> Self {
        Self {
            state: StatusState::default(),
            message: None,
            spinner_frame: 0,
            spinner_style: SpinnerStyle::Dots,
            input_tokens: None,
            output_tokens: None,
            cost: None,
            context_percent: None,
            elapsed: None,
            elapsed_threshold: Duration::from_secs(30),
            direction: DataDirection::default(),
            offline: false,
            cwd: None,
            cwd_max_width: 20,
        }
    }

    /// Set the status state.
    pub fn state(mut self, state: StatusState) -> Self {
        self.state = state;
        self
    }

    /// Set a custom message (overrides default state label).
    pub fn message(mut self, msg: impl Into<String>) -> Self {
        self.message = Some(msg.into());
        self
    }

    /// Set the spinner style for active states.
    pub fn spinner_style(mut self, style: SpinnerStyle) -> Self {
        self.spinner_style = style;
        self
    }

    /// Set token counts (input and output).
    ///
    /// Displayed with compact notation: 1.5k, 10.0m, 125.3k
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::TokenStatusBar;
    ///
    /// let status = TokenStatusBar::new()
    ///     .tokens(1523, 4891);
    /// ```
    pub fn tokens(mut self, input: u64, output: u64) -> Self {
        self.input_tokens = Some(input);
        self.output_tokens = Some(output);
        self
    }

    /// Set input token count only.
    pub fn input_tokens(mut self, tokens: u64) -> Self {
        self.input_tokens = Some(tokens);
        self
    }

    /// Set output token count only.
    pub fn output_tokens(mut self, tokens: u64) -> Self {
        self.output_tokens = Some(tokens);
        self
    }

    /// Set the API cost in dollars.
    ///
    /// Displayed with adaptive formatting:
    /// - Under $0.01: "$0.00"
    /// - Under $1.00: "$0.05" (2 decimal places)
    /// - $1.00 and over: "$1.23" (2 decimal places)
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::TokenStatusBar;
    ///
    /// let status = TokenStatusBar::new()
    ///     .cost(0.0523);
    /// ```
    pub fn cost(mut self, cost: f64) -> Self {
        self.cost = Some(cost);
        self
    }

    /// Set the context window usage percentage.
    ///
    /// - Yellow warning at 60%
    /// - Red warning at 80%
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::TokenStatusBar;
    ///
    /// let status = TokenStatusBar::new()
    ///     .context_percent(65.0);  // Shows yellow warning
    /// ```
    pub fn context_percent(mut self, percent: f64) -> Self {
        self.context_percent = Some(percent.clamp(0.0, 100.0));
        self
    }

    /// Set the elapsed time since operation started.
    ///
    /// Only displayed if elapsed exceeds the threshold (default 30s).
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::TokenStatusBar;
    /// use std::time::Duration;
    ///
    /// let status = TokenStatusBar::new()
    ///     .elapsed(Duration::from_secs(45));  // Shows "0:45"
    /// ```
    pub fn elapsed(mut self, elapsed: Duration) -> Self {
        self.elapsed = Some(elapsed);
        self
    }

    /// Set the threshold for displaying elapsed time (default 30s).
    pub fn elapsed_threshold(mut self, threshold: Duration) -> Self {
        self.elapsed_threshold = threshold;
        self
    }

    /// Set the data direction indicator.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::{TokenStatusBar, DataDirection};
    ///
    /// let status = TokenStatusBar::new()
    ///     .direction(DataDirection::Receiving);  // Shows "↓"
    /// ```
    pub fn direction(mut self, direction: DataDirection) -> Self {
        self.direction = direction;
        self
    }

    /// Set the offline status.
    ///
    /// When true, displays "offline" in error color.
    pub fn offline(mut self, offline: bool) -> Self {
        self.offline = offline;
        self
    }

    /// Set the current working directory.
    ///
    /// Automatically truncated if too long. Paths starting with home directory
    /// are shortened to `~/...`.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::TokenStatusBar;
    ///
    /// let status = TokenStatusBar::new()
    ///     .cwd("/Users/user/projects/myapp");
    /// ```
    pub fn cwd(mut self, cwd: impl Into<String>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Set the maximum width for cwd display (default 20).
    pub fn cwd_max_width(mut self, width: usize) -> Self {
        self.cwd_max_width = width;
        self
    }

    /// Advance the spinner animation by one frame.
    pub fn tick(&mut self) {
        if self.state.is_active() {
            self.spinner_frame = self.spinner_frame.wrapping_add(1);
        }
    }

    /// Get the current status state.
    pub fn current_state(&self) -> StatusState {
        self.state
    }

    /// Get the current message (custom or default).
    pub fn current_message(&self) -> &str {
        self.message.as_deref().unwrap_or(self.state.label())
    }

    /// Format a token count with compact notation.
    ///
    /// - Under 1000: "523"
    /// - Under 10000: "1.5k"
    /// - Under 1M: "125.3k"
    /// - 1M and over: "1.2m"
    fn format_tokens(count: u64) -> String {
        if count < 1000 {
            format!("{}", count)
        } else if count < 10_000 {
            format!("{:.1}k", count as f64 / 1000.0)
        } else if count < 1_000_000 {
            format!("{:.1}k", count as f64 / 1000.0)
        } else {
            format!("{:.1}m", count as f64 / 1_000_000.0)
        }
    }

    /// Format cost with adaptive decimal places.
    fn format_cost(cost: f64) -> String {
        if cost < 0.01 {
            "$0.00".to_string()
        } else {
            format!("${:.2}", cost)
        }
    }

    /// Format elapsed time as M:SS or H:MM:SS.
    fn format_elapsed(duration: Duration) -> String {
        let total_secs = duration.as_secs();
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        let secs = total_secs % 60;

        if hours > 0 {
            format!("{}:{:02}:{:02}", hours, mins, secs)
        } else {
            format!("{}:{:02}", mins, secs)
        }
    }

    /// Format cwd with truncation and home directory shortening.
    fn format_cwd(&self, cwd: &str) -> String {
        // Try to shorten home directory
        let shortened = if let Ok(home) = std::env::var("HOME") {
            if cwd.starts_with(&home) {
                format!("~{}", &cwd[home.len()..])
            } else {
                cwd.to_string()
            }
        } else {
            cwd.to_string()
        };

        // Truncate if too long
        if shortened.len() > self.cwd_max_width {
            let start = shortened.len() - self.cwd_max_width + 3;
            format!("...{}", &shortened[start..])
        } else {
            shortened
        }
    }

    /// Get the color for context percentage.
    fn context_color(percent: f64) -> Color {
        if percent >= 80.0 {
            Color::Red
        } else if percent >= 60.0 {
            Color::Yellow
        } else {
            Color::White
        }
    }

    /// Get the warning symbol for context percentage.
    fn context_warning(percent: f64) -> &'static str {
        if percent >= 80.0 {
            " ⚠"
        } else if percent >= 60.0 {
            " ⚠"
        } else {
            ""
        }
    }

    /// Convert to a Node for rendering.
    pub fn to_node(&self) -> Node {
        let state_color = self.state.color();
        let message = self.current_message();

        let mut container = BoxNode::new().flex_direction(FlexDirection::Row);

        // Status indicator (spinner or symbol)
        let indicator: Node = if self.state.is_active() {
            let spinner_text = self.spinner_style.frame(self.spinner_frame);
            TextNode::new(spinner_text).color(state_color).into()
        } else {
            TextNode::new(self.state.indicator())
                .color(state_color)
                .into()
        };
        container = container.child(indicator);
        container = container.child(TextNode::new(" "));
        container = container.child(TextNode::new(format!("{}...", message)).color(state_color));

        // Tokens
        if let (Some(input), Some(output)) = (self.input_tokens, self.output_tokens) {
            container = container.child(TextNode::new(" | ").color(Color::BrightBlack));
            container = container.child(TextNode::new(format!(
                "{}/{} tokens",
                Self::format_tokens(input),
                Self::format_tokens(output)
            )));
        }

        // Cost
        if let Some(cost) = self.cost {
            container = container.child(TextNode::new(" | ").color(Color::BrightBlack));
            container = container.child(TextNode::new(Self::format_cost(cost)).color(Color::Green));
        }

        // Context percentage
        if let Some(percent) = self.context_percent {
            let color = Self::context_color(percent);
            let warning = Self::context_warning(percent);
            container = container.child(TextNode::new(" | ").color(Color::BrightBlack));
            container =
                container.child(TextNode::new(format!("{:.0}%{}", percent, warning)).color(color));
        }

        // Elapsed time (only if above threshold)
        if let Some(elapsed) = self.elapsed {
            if elapsed >= self.elapsed_threshold {
                container = container.child(TextNode::new(" | ").color(Color::BrightBlack));
                container = container.child(TextNode::new(Self::format_elapsed(elapsed)));
            }
        }

        // Direction indicator
        if self.direction != DataDirection::Idle {
            container = container.child(TextNode::new(" | ").color(Color::BrightBlack));
            container = container.child(TextNode::new(self.direction.symbol()));
        }

        // Offline indicator
        if self.offline {
            container = container.child(TextNode::new(" | ").color(Color::BrightBlack));
            container = container.child(TextNode::new("offline").color(Color::Red));
        }

        // Working directory
        if let Some(ref cwd) = self.cwd {
            container = container.child(TextNode::new(" | ").color(Color::BrightBlack));
            container = container.child(TextNode::new(self.format_cwd(cwd)).color(Color::Cyan));
        }

        container.into()
    }

    /// Render Tier 0: Plain text without any formatting.
    fn render_tier0(&self) -> Node {
        let state_text = match self.state {
            StatusState::Idle => "IDLE",
            StatusState::Thinking => "THINKING",
            StatusState::Executing => "EXECUTING",
            StatusState::Error => "ERROR",
        };

        let mut parts = vec![format!("[{}]", state_text)];

        if let (Some(input), Some(output)) = (self.input_tokens, self.output_tokens) {
            parts.push(format!(
                "{}/{} tokens",
                Self::format_tokens(input),
                Self::format_tokens(output)
            ));
        }

        if let Some(cost) = self.cost {
            parts.push(Self::format_cost(cost));
        }

        if let Some(percent) = self.context_percent {
            parts.push(format!("{:.0}%", percent));
        }

        if let Some(elapsed) = self.elapsed {
            if elapsed >= self.elapsed_threshold {
                parts.push(Self::format_elapsed(elapsed));
            }
        }

        if self.offline {
            parts.push("OFFLINE".to_string());
        }

        TextNode::new(parts.join(" | ")).into()
    }

    /// Render Tier 1: ASCII indicators with basic formatting.
    fn render_tier1(&self) -> Node {
        let indicator = match self.state {
            StatusState::Idle => "[+]",
            StatusState::Thinking => {
                const FRAMES: &[&str] = &["[-]", "[\\]", "[|]", "[/]"];
                FRAMES[self.spinner_frame % FRAMES.len()]
            }
            StatusState::Executing => {
                const FRAMES: &[&str] = &["[.]", "[..]", "[...]", "[..]"];
                FRAMES[self.spinner_frame % FRAMES.len()]
            }
            StatusState::Error => "[!]",
        };

        let mut container = BoxNode::new().flex_direction(FlexDirection::Row);
        container = container.child(TextNode::new(indicator));
        container = container.child(TextNode::new(" "));
        container = container.child(TextNode::new(self.current_message()));

        if let (Some(input), Some(output)) = (self.input_tokens, self.output_tokens) {
            container = container.child(TextNode::new(" | "));
            container = container.child(TextNode::new(format!(
                "{}/{} tokens",
                Self::format_tokens(input),
                Self::format_tokens(output)
            )));
        }

        if let Some(cost) = self.cost {
            container = container.child(TextNode::new(" | "));
            container = container.child(TextNode::new(Self::format_cost(cost)));
        }

        if let Some(percent) = self.context_percent {
            let warning = if percent >= 60.0 { " (!)" } else { "" };
            container = container.child(TextNode::new(" | "));
            container = container.child(TextNode::new(format!("{:.0}%{}", percent, warning)));
        }

        if let Some(elapsed) = self.elapsed {
            if elapsed >= self.elapsed_threshold {
                container = container.child(TextNode::new(" | "));
                container = container.child(TextNode::new(Self::format_elapsed(elapsed)));
            }
        }

        if self.direction != DataDirection::Idle {
            container = container.child(TextNode::new(" | "));
            container = container.child(TextNode::new(self.direction.ascii_symbol()));
        }

        if self.offline {
            container = container.child(TextNode::new(" | OFFLINE"));
        }

        if let Some(ref cwd) = self.cwd {
            container = container.child(TextNode::new(" | "));
            container = container.child(TextNode::new(self.format_cwd(cwd)));
        }

        container.into()
    }
}

impl AdaptiveComponent for TokenStatusBar {
    fn render_for_tier(&self, tier: RenderTier) -> Node {
        match tier {
            RenderTier::Tier0Fallback => self.render_tier0(),
            RenderTier::Tier1Ansi => self.render_tier1(),
            RenderTier::Tier2Retained | RenderTier::Tier3Gpu => self.to_node(),
        }
    }

    fn tier_features(&self) -> TierFeatures {
        TierFeatures::new("TokenStatusBar")
            .tier0("Plain text with token counts and metrics")
            .tier1("ASCII indicators with basic metrics")
            .tier2("Unicode indicators with colors, spinners, and full metrics")
            .tier3("Full rendering with GPU acceleration")
    }

    fn minimum_tier(&self) -> Option<RenderTier> {
        None // Works at all tiers
    }
}

impl From<TokenStatusBar> for Node {
    fn from(status_bar: TokenStatusBar) -> Self {
        status_bar.to_node()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn default_state_is_idle() {
        let status = StatusBar::new();
        assert_eq!(status.current_state(), StatusState::Idle);
    }

    #[test]
    fn state_is_active_for_thinking_and_executing() {
        assert!(!StatusState::Idle.is_active());
        assert!(StatusState::Thinking.is_active());
        assert!(StatusState::Executing.is_active());
        assert!(!StatusState::Error.is_active());
    }

    #[test]
    fn state_colors_are_correct() {
        assert_eq!(StatusState::Idle.color(), Color::Green);
        assert_eq!(StatusState::Thinking.color(), Color::Yellow);
        assert_eq!(StatusState::Executing.color(), Color::Blue);
        assert_eq!(StatusState::Error.color(), Color::Red);
    }

    #[test]
    fn state_labels_are_correct() {
        assert_eq!(StatusState::Idle.label(), "Ready");
        assert_eq!(StatusState::Thinking.label(), "Thinking");
        assert_eq!(StatusState::Executing.label(), "Executing");
        assert_eq!(StatusState::Error.label(), "Error");
    }

    #[test]
    fn state_indicators_are_correct() {
        assert_eq!(StatusState::Idle.indicator(), "●");
        assert_eq!(StatusState::Error.indicator(), "✗");
    }

    #[test]
    fn custom_message_overrides_default() {
        let status = StatusBar::new()
            .state(StatusState::Thinking)
            .message("Custom thinking message");

        assert_eq!(status.current_message(), "Custom thinking message");
    }

    #[test]
    fn default_message_uses_state_label() {
        let status = StatusBar::new().state(StatusState::Executing);
        assert_eq!(status.current_message(), "Executing");
    }

    #[test]
    fn tick_advances_spinner_for_active_states() {
        let mut status = StatusBar::new().state(StatusState::Thinking);
        assert_eq!(status.spinner_frame(), 0);

        status.tick();
        assert_eq!(status.spinner_frame(), 1);

        status.tick();
        assert_eq!(status.spinner_frame(), 2);
    }

    #[test]
    fn tick_does_not_advance_for_inactive_states() {
        let mut status = StatusBar::new().state(StatusState::Idle);
        assert_eq!(status.spinner_frame(), 0);

        status.tick();
        assert_eq!(status.spinner_frame(), 0);

        let mut error_status = StatusBar::new().state(StatusState::Error);
        error_status.tick();
        assert_eq!(error_status.spinner_frame(), 0);
    }

    #[test]
    fn spinner_style_can_be_changed() {
        let status = StatusBar::new()
            .state(StatusState::Executing)
            .spinner_style(SpinnerStyle::Circle);

        // Verify the node can be created without panicking
        let _node = status.to_node();
    }

    #[test]
    fn to_node_returns_valid_node_for_all_states() {
        for state in [
            StatusState::Idle,
            StatusState::Thinking,
            StatusState::Executing,
            StatusState::Error,
        ] {
            let status = StatusBar::new().state(state);
            let _node = status.to_node();
        }
    }

    #[test]
    fn from_trait_works() {
        let status = StatusBar::new().state(StatusState::Thinking);
        let _node: Node = status.into();
    }

    #[test]
    fn default_trait_works() {
        let status = StatusBar::default();
        assert_eq!(status.current_state(), StatusState::Idle);
    }

    #[test]
    fn status_state_default_is_idle() {
        let state = StatusState::default();
        assert_eq!(state, StatusState::Idle);
    }

    #[test]
    fn spinner_frame_wraps_around() {
        let mut status = StatusBar::new().state(StatusState::Thinking);

        // Tick many times past the frame count
        for _ in 0..100 {
            status.tick();
        }

        // Should not panic and frame should wrap around
        let _node = status.to_node();
    }

    // Adaptive rendering tests

    fn node_to_text(node: &Node) -> String {
        match node {
            Node::Text(t) => t.content.to_string(),
            Node::Box(b) => {
                let mut result = String::new();
                for c in &b.children {
                    result.push_str(&node_to_text(c));
                }
                result
            }
            Node::Root(r) => {
                let mut result = String::new();
                for c in &r.children {
                    result.push_str(&node_to_text(c));
                }
                result
            }
            Node::Static(s) => {
                let mut result = String::new();
                for c in &s.children {
                    result.push_str(&node_to_text(c));
                }
                result
            }
            Node::Custom(c) => {
                let mut result = String::new();
                for child in c.widget().children() {
                    result.push_str(&node_to_text(child));
                }
                result
            }
        }
    }

    #[test]
    fn test_adaptive_tier0_idle() {
        let status = StatusBar::new().state(StatusState::Idle);
        let node = status.render_for_tier(RenderTier::Tier0Fallback);
        let text = node_to_text(&node);

        assert!(text.contains("[STATUS:"));
        assert!(text.contains("IDLE"));
        assert!(text.contains("Ready"));
    }

    #[test]
    fn test_adaptive_tier0_thinking() {
        let status = StatusBar::new().state(StatusState::Thinking);
        let node = status.render_for_tier(RenderTier::Tier0Fallback);
        let text = node_to_text(&node);

        assert!(text.contains("THINKING"));
    }

    #[test]
    fn test_adaptive_tier0_error() {
        let status = StatusBar::new().state(StatusState::Error);
        let node = status.render_for_tier(RenderTier::Tier0Fallback);
        let text = node_to_text(&node);

        assert!(text.contains("ERROR"));
    }

    #[test]
    fn test_adaptive_tier0_custom_message() {
        let status = StatusBar::new()
            .state(StatusState::Thinking)
            .message("Processing...");
        let node = status.render_for_tier(RenderTier::Tier0Fallback);
        let text = node_to_text(&node);

        assert!(text.contains("Processing..."));
    }

    #[test]
    fn test_adaptive_tier1_idle() {
        let status = StatusBar::new().state(StatusState::Idle);
        let node = status.render_for_tier(RenderTier::Tier1Ansi);
        let text = node_to_text(&node);

        assert!(text.contains("[+]"));
        assert!(text.contains("Ready"));
    }

    #[test]
    fn test_adaptive_tier1_error() {
        let status = StatusBar::new().state(StatusState::Error);
        let node = status.render_for_tier(RenderTier::Tier1Ansi);
        let text = node_to_text(&node);

        assert!(text.contains("[!]"));
    }

    #[test]
    fn test_adaptive_tier1_spinner_animation() {
        let mut status = StatusBar::new().state(StatusState::Thinking);

        // Get first frame
        let node1 = status.render_for_tier(RenderTier::Tier1Ansi);
        let text1 = node_to_text(&node1);

        // Advance and get second frame
        status.tick();
        let node2 = status.render_for_tier(RenderTier::Tier1Ansi);
        let text2 = node_to_text(&node2);

        // Frames should differ (animation is working)
        assert_ne!(text1, text2);
    }

    #[test]
    fn test_adaptive_tier2_uses_unicode() {
        let status = StatusBar::new().state(StatusState::Idle);
        let node = status.render_for_tier(RenderTier::Tier2Retained);
        let text = node_to_text(&node);

        // Should contain Unicode indicator
        assert!(text.contains("●"));
    }

    #[test]
    fn test_adaptive_tier2_same_as_default() {
        let status = StatusBar::new().state(StatusState::Idle);

        let tier2_node = status.render_for_tier(RenderTier::Tier2Retained);
        let default_node = status.to_node();

        assert_eq!(node_to_text(&tier2_node), node_to_text(&default_node));
    }

    #[test]
    fn test_adaptive_tier3_same_as_tier2() {
        let status = StatusBar::new().state(StatusState::Error);

        let tier2_node = status.render_for_tier(RenderTier::Tier2Retained);
        let tier3_node = status.render_for_tier(RenderTier::Tier3Gpu);

        assert_eq!(node_to_text(&tier2_node), node_to_text(&tier3_node));
    }

    #[test]
    fn test_adaptive_tier_features() {
        let status = StatusBar::new();
        let features = status.tier_features();

        assert_eq!(features.name, Some("StatusBar"));
        assert!(features.tier0_description.is_some());
        assert!(features.tier1_description.is_some());
        assert!(features.tier2_description.is_some());
        assert!(features.tier3_description.is_some());
    }

    #[test]
    fn test_adaptive_minimum_tier() {
        let status = StatusBar::new();
        assert_eq!(status.minimum_tier(), None);
        assert!(status.supports_tier(RenderTier::Tier0Fallback));
        assert!(status.supports_tier(RenderTier::Tier3Gpu));
    }

    #[test]
    fn test_adaptive_all_states_all_tiers() {
        let states = [
            StatusState::Idle,
            StatusState::Thinking,
            StatusState::Executing,
            StatusState::Error,
        ];
        let tiers = [
            RenderTier::Tier0Fallback,
            RenderTier::Tier1Ansi,
            RenderTier::Tier2Retained,
            RenderTier::Tier3Gpu,
        ];

        for state in states {
            for tier in tiers {
                let status = StatusBar::new().state(state);
                let _node = status.render_for_tier(tier);
                // Should not panic
            }
        }
    }

    // =========================================================================
    // TokenStatusBar tests
    // =========================================================================

    #[test]
    fn token_status_bar_default() {
        let status = TokenStatusBar::new();
        assert_eq!(status.current_state(), StatusState::Idle);
        assert_eq!(status.current_message(), "Ready");
    }

    #[test]
    fn token_status_bar_with_tokens() {
        let status = TokenStatusBar::new()
            .state(StatusState::Thinking)
            .tokens(1523, 4891);

        let node = status.to_node();
        let text = node_to_text(&node);

        assert!(text.contains("Thinking"));
        assert!(text.contains("1.5k"));
        assert!(text.contains("4.9k"));
        assert!(text.contains("tokens"));
    }

    #[test]
    fn token_status_bar_format_tokens() {
        assert_eq!(TokenStatusBar::format_tokens(500), "500");
        assert_eq!(TokenStatusBar::format_tokens(1523), "1.5k");
        assert_eq!(TokenStatusBar::format_tokens(12345), "12.3k");
        assert_eq!(TokenStatusBar::format_tokens(125300), "125.3k");
        assert_eq!(TokenStatusBar::format_tokens(1_500_000), "1.5m");
    }

    #[test]
    fn token_status_bar_format_cost() {
        assert_eq!(TokenStatusBar::format_cost(0.001), "$0.00");
        assert_eq!(TokenStatusBar::format_cost(0.05), "$0.05");
        assert_eq!(TokenStatusBar::format_cost(0.523), "$0.52");
        assert_eq!(TokenStatusBar::format_cost(1.23), "$1.23");
        assert_eq!(TokenStatusBar::format_cost(12.345), "$12.35");
    }

    #[test]
    fn token_status_bar_format_elapsed() {
        assert_eq!(
            TokenStatusBar::format_elapsed(Duration::from_secs(45)),
            "0:45"
        );
        assert_eq!(
            TokenStatusBar::format_elapsed(Duration::from_secs(125)),
            "2:05"
        );
        assert_eq!(
            TokenStatusBar::format_elapsed(Duration::from_secs(3661)),
            "1:01:01"
        );
    }

    #[test]
    fn token_status_bar_elapsed_threshold() {
        // Below threshold - should not show
        let status = TokenStatusBar::new()
            .elapsed(Duration::from_secs(20))
            .elapsed_threshold(Duration::from_secs(30));

        let node = status.to_node();
        let text = node_to_text(&node);
        assert!(!text.contains("0:20"));

        // Above threshold - should show
        let status = TokenStatusBar::new()
            .elapsed(Duration::from_secs(45))
            .elapsed_threshold(Duration::from_secs(30));

        let node = status.to_node();
        let text = node_to_text(&node);
        assert!(text.contains("0:45"));
    }

    #[test]
    fn token_status_bar_context_percent() {
        // Normal - no warning
        let status = TokenStatusBar::new().context_percent(40.0);
        let node = status.to_node();
        let text = node_to_text(&node);
        assert!(text.contains("40%"));
        assert!(!text.contains("⚠"));

        // Yellow warning at 60%
        let status = TokenStatusBar::new().context_percent(65.0);
        let node = status.to_node();
        let text = node_to_text(&node);
        assert!(text.contains("65%"));
        assert!(text.contains("⚠"));

        // Red warning at 80%
        let status = TokenStatusBar::new().context_percent(85.0);
        let node = status.to_node();
        let text = node_to_text(&node);
        assert!(text.contains("85%"));
        assert!(text.contains("⚠"));
    }

    #[test]
    fn token_status_bar_direction() {
        assert_eq!(DataDirection::Idle.symbol(), "•");
        assert_eq!(DataDirection::Sending.symbol(), "↑");
        assert_eq!(DataDirection::Receiving.symbol(), "↓");

        let status = TokenStatusBar::new().direction(DataDirection::Receiving);
        let node = status.to_node();
        let text = node_to_text(&node);
        assert!(text.contains("↓"));
    }

    #[test]
    fn token_status_bar_offline() {
        let status = TokenStatusBar::new().offline(true);
        let node = status.to_node();
        let text = node_to_text(&node);
        assert!(text.contains("offline"));
    }

    #[test]
    fn token_status_bar_cwd() {
        let status = TokenStatusBar::new().cwd("/short/path");
        let node = status.to_node();
        let text = node_to_text(&node);
        assert!(text.contains("/short/path"));
    }

    #[test]
    fn token_status_bar_full_example() {
        let status = TokenStatusBar::new()
            .state(StatusState::Thinking)
            .tokens(1523, 4891)
            .cost(0.0523)
            .context_percent(65.0)
            .elapsed(Duration::from_secs(45))
            .elapsed_threshold(Duration::from_secs(30))
            .direction(DataDirection::Receiving)
            .cwd("/Users/user/project");

        let node = status.to_node();
        let text = node_to_text(&node);

        assert!(text.contains("Thinking"));
        assert!(text.contains("1.5k"));
        assert!(text.contains("4.9k"));
        assert!(text.contains("$0.05"));
        assert!(text.contains("65%"));
        assert!(text.contains("0:45"));
        assert!(text.contains("↓"));
    }

    #[test]
    fn token_status_bar_tick() {
        let mut status = TokenStatusBar::new().state(StatusState::Thinking);
        let frame1 = status.spinner_frame;

        status.tick();
        let frame2 = status.spinner_frame;

        assert_eq!(frame2, frame1 + 1);
    }

    #[test]
    fn token_status_bar_adaptive_rendering() {
        let status = TokenStatusBar::new()
            .state(StatusState::Thinking)
            .tokens(1000, 2000)
            .cost(0.50);

        // All tiers should work without panic
        let _tier0 = status.render_for_tier(RenderTier::Tier0Fallback);
        let _tier1 = status.render_for_tier(RenderTier::Tier1Ansi);
        let _tier2 = status.render_for_tier(RenderTier::Tier2Retained);
        let _tier3 = status.render_for_tier(RenderTier::Tier3Gpu);
    }

    #[test]
    fn token_status_bar_tier0_contains_metrics() {
        let status = TokenStatusBar::new()
            .state(StatusState::Executing)
            .tokens(500, 1000)
            .cost(0.25);

        let node = status.render_for_tier(RenderTier::Tier0Fallback);
        let text = node_to_text(&node);

        assert!(text.contains("[EXECUTING]"));
        assert!(text.contains("500"));
        assert!(text.contains("1.0k"));
        assert!(text.contains("$0.25"));
    }
}
