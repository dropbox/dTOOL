//! Animation API for smooth property transitions.
//!
//! This module provides tools for animating values over time with various
//! easing functions. Animations can be used to create smooth transitions
//! for UI properties like position, size, opacity, and colors.
//!
//! # Example
//!
//! ```rust
//! use inky::animation::{Animation, Easing};
//! use std::time::Duration;
//!
//! // Create an animation from 0.0 to 100.0 over 500ms
//! let anim = Animation::new(0.0, 100.0, Duration::from_millis(500))
//!     .easing(Easing::EaseOut);
//!
//! // Get value at a specific time
//! let value = anim.value_at(Duration::from_millis(250));
//! ```
//!
//! # Easing Functions
//!
//! The module provides common easing functions:
//!
//! - [`Easing::Linear`] - Constant rate of change
//! - [`Easing::EaseIn`] - Starts slow, accelerates
//! - [`Easing::EaseOut`] - Starts fast, decelerates
//! - [`Easing::EaseInOut`] - Slow start and end, fast middle
//! - [`Easing::Bounce`] - Bouncy effect at the end
//! - [`Easing::Elastic`] - Spring-like overshoot
//!
//! [`Easing::Linear`]: crate::animation::Easing::Linear
//! [`Easing::EaseIn`]: crate::animation::Easing::EaseIn
//! [`Easing::EaseOut`]: crate::animation::Easing::EaseOut
//! [`Easing::EaseInOut`]: crate::animation::Easing::EaseInOut
//! [`Easing::Bounce`]: crate::animation::Easing::Bounce
//! [`Easing::Elastic`]: crate::animation::Easing::Elastic

use std::time::{Duration, Instant};

/// Easing function types for animations.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Easing {
    /// Linear interpolation - constant rate.
    #[default]
    Linear,
    /// Quadratic ease-in - starts slow.
    EaseIn,
    /// Quadratic ease-out - ends slow.
    EaseOut,
    /// Quadratic ease-in-out - slow start and end.
    EaseInOut,
    /// Cubic ease-in - more pronounced slow start.
    CubicIn,
    /// Cubic ease-out - more pronounced slow end.
    CubicOut,
    /// Cubic ease-in-out.
    CubicInOut,
    /// Bounce effect at the end.
    Bounce,
    /// Elastic spring effect.
    Elastic,
}

impl Easing {
    /// Apply the easing function to a normalized time value (0.0 to 1.0).
    ///
    /// # Arguments
    ///
    /// * `t` - Normalized time from 0.0 (start) to 1.0 (end)
    ///
    /// # Returns
    ///
    /// Eased value, typically 0.0 to 1.0 but may overshoot for elastic easing.
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);

        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t,
            Easing::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Easing::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Easing::CubicIn => t * t * t,
            Easing::CubicOut => 1.0 - (1.0 - t).powi(3),
            Easing::CubicInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Easing::Bounce => {
                let n1 = 7.5625;
                let d1 = 2.75;
                let mut t = t;

                if t < 1.0 / d1 {
                    n1 * t * t
                } else if t < 2.0 / d1 {
                    t -= 1.5 / d1;
                    n1 * t * t + 0.75
                } else if t < 2.5 / d1 {
                    t -= 2.25 / d1;
                    n1 * t * t + 0.9375
                } else {
                    t -= 2.625 / d1;
                    n1 * t * t + 0.984375
                }
            }
            Easing::Elastic => {
                if t == 0.0 || t == 1.0 {
                    t
                } else {
                    let c4 = (2.0 * std::f32::consts::PI) / 3.0;
                    -2.0_f32.powf(10.0 * t - 10.0) * ((t * 10.0 - 10.75) * c4).sin()
                }
            }
        }
    }
}

/// An animation that interpolates between two values over time.
///
/// # Example
///
/// ```rust
/// use inky::animation::{Animation, Easing};
/// use std::time::Duration;
///
/// let anim = Animation::new(0.0, 1.0, Duration::from_millis(300));
/// assert_eq!(anim.value_at(Duration::ZERO), 0.0);
/// assert_eq!(anim.value_at(Duration::from_millis(300)), 1.0);
/// ```
#[derive(Debug, Clone)]
pub struct Animation {
    /// Starting value.
    from: f32,
    /// Ending value.
    to: f32,
    /// Duration of the animation.
    duration: Duration,
    /// Easing function to use.
    easing: Easing,
    /// Delay before animation starts.
    delay: Duration,
    /// Whether animation repeats.
    repeat: bool,
    /// Whether animation alternates direction on repeat.
    alternate: bool,
}

impl Animation {
    /// Create a new animation.
    ///
    /// # Arguments
    ///
    /// * `from` - Starting value
    /// * `to` - Ending value
    /// * `duration` - How long the animation takes
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::animation::Animation;
    /// use std::time::Duration;
    ///
    /// let anim = Animation::new(0.0, 100.0, Duration::from_secs(1));
    /// ```
    pub fn new(from: f32, to: f32, duration: Duration) -> Self {
        Self {
            from,
            to,
            duration,
            easing: Easing::default(),
            delay: Duration::ZERO,
            repeat: false,
            alternate: false,
        }
    }

    /// Set the easing function.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::animation::{Animation, Easing};
    /// use std::time::Duration;
    ///
    /// let anim = Animation::new(0.0, 1.0, Duration::from_millis(500))
    ///     .easing(Easing::EaseOut);
    /// ```
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Set a delay before the animation starts.
    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// Make the animation repeat indefinitely.
    pub fn repeat(mut self) -> Self {
        self.repeat = true;
        self
    }

    /// Make the animation alternate direction on each repeat.
    ///
    /// Only has effect if `repeat()` is also called.
    pub fn alternate(mut self) -> Self {
        self.alternate = true;
        self
    }

    /// Get the value at a specific elapsed time.
    ///
    /// # Arguments
    ///
    /// * `elapsed` - Time since animation started
    ///
    /// # Returns
    ///
    /// The interpolated value at that time.
    pub fn value_at(&self, elapsed: Duration) -> f32 {
        // Handle delay
        if elapsed < self.delay {
            return self.from;
        }

        let elapsed = elapsed - self.delay;

        if self.duration.is_zero() {
            return self.to;
        }

        let elapsed_secs = elapsed.as_secs_f32();
        let duration_secs = self.duration.as_secs_f32();

        let mut t = elapsed_secs / duration_secs;

        if self.repeat {
            // Handle repeating
            let cycle = (t as u32) % 2;
            t = t.fract();

            // Alternate direction on odd cycles
            if self.alternate && cycle == 1 {
                t = 1.0 - t;
            }
        } else {
            t = t.min(1.0);
        }

        let eased = self.easing.apply(t);
        self.from + (self.to - self.from) * eased
    }

    /// Check if the animation has completed.
    ///
    /// Always returns `false` for repeating animations.
    pub fn is_complete(&self, elapsed: Duration) -> bool {
        if self.repeat {
            return false;
        }
        elapsed >= self.delay + self.duration
    }

    /// Get the total duration including delay.
    pub fn total_duration(&self) -> Duration {
        self.delay + self.duration
    }

    /// Get the starting value.
    pub fn from(&self) -> f32 {
        self.from
    }

    /// Get the ending value.
    pub fn to(&self) -> f32 {
        self.to
    }
}

/// A running animation with timing.
///
/// This wraps an [`Animation`] with an [`Instant`] to track when it started.
///
/// # Example
///
/// ```rust,ignore
/// use inky::animation::{Animation, AnimationState};
/// use std::time::Duration;
///
/// let anim = Animation::new(0.0, 100.0, Duration::from_millis(500));
/// let mut state = AnimationState::start(anim);
///
/// // In your render loop:
/// let current_value = state.value();
/// if state.is_complete() {
///     // Animation finished
/// }
/// ```
#[derive(Debug, Clone)]
pub struct AnimationState {
    /// The animation configuration.
    animation: Animation,
    /// When the animation started.
    started: Instant,
}

impl AnimationState {
    /// Start an animation now.
    pub fn start(animation: Animation) -> Self {
        Self {
            animation,
            started: Instant::now(),
        }
    }

    /// Start an animation at a specific instant.
    pub fn start_at(animation: Animation, instant: Instant) -> Self {
        Self {
            animation,
            started: instant,
        }
    }

    /// Get the current animated value.
    pub fn value(&self) -> f32 {
        self.animation.value_at(self.started.elapsed())
    }

    /// Check if the animation has completed.
    pub fn is_complete(&self) -> bool {
        self.animation.is_complete(self.started.elapsed())
    }

    /// Get elapsed time since start.
    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// Reset the animation to start again.
    pub fn reset(&mut self) {
        self.started = Instant::now();
    }

    /// Get a reference to the underlying animation.
    pub fn animation(&self) -> &Animation {
        &self.animation
    }
}

/// Animate a color transition.
///
/// Interpolates RGB values between two colors.
pub fn lerp_color(
    from: crate::style::Color,
    to: crate::style::Color,
    t: f32,
) -> crate::style::Color {
    use crate::style::Color;

    let t = t.clamp(0.0, 1.0);

    // Convert colors to RGB
    let (r1, g1, b1) = color_to_rgb(from);
    let (r2, g2, b2) = color_to_rgb(to);

    // Interpolate
    let r = lerp_u8(r1, r2, t);
    let g = lerp_u8(g1, g2, t);
    let b = lerp_u8(b1, b2, t);

    Color::Rgb(r, g, b)
}

/// Convert a Color to RGB tuple.
fn color_to_rgb(color: crate::style::Color) -> (u8, u8, u8) {
    use crate::style::Color;

    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::Red => (205, 49, 49),
        Color::Green => (13, 188, 121),
        Color::Yellow => (229, 229, 16),
        Color::Blue => (36, 114, 200),
        Color::Magenta => (188, 63, 188),
        Color::Cyan => (17, 168, 205),
        Color::White => (229, 229, 229),
        Color::BrightBlack => (102, 102, 102),
        Color::BrightRed => (241, 76, 76),
        Color::BrightGreen => (35, 209, 139),
        Color::BrightYellow => (245, 245, 67),
        Color::BrightBlue => (59, 142, 234),
        Color::BrightMagenta => (214, 112, 214),
        Color::BrightCyan => (41, 184, 219),
        Color::BrightWhite => (229, 229, 229),
        Color::Ansi256(n) => ansi256_to_rgb(n),
        Color::Default => (229, 229, 229), // Default to white-ish
    }
}

/// Convert ANSI 256 color to RGB.
fn ansi256_to_rgb(n: u8) -> (u8, u8, u8) {
    if n < 16 {
        // Standard colors
        match n {
            0 => (0, 0, 0),
            1 => (128, 0, 0),
            2 => (0, 128, 0),
            3 => (128, 128, 0),
            4 => (0, 0, 128),
            5 => (128, 0, 128),
            6 => (0, 128, 128),
            7 => (192, 192, 192),
            8 => (128, 128, 128),
            9 => (255, 0, 0),
            10 => (0, 255, 0),
            11 => (255, 255, 0),
            12 => (0, 0, 255),
            13 => (255, 0, 255),
            14 => (0, 255, 255),
            15 => (255, 255, 255),
            _ => (0, 0, 0),
        }
    } else if n < 232 {
        // 6x6x6 color cube
        let n = n - 16;
        let r = (n / 36) % 6;
        let g = (n / 6) % 6;
        let b = n % 6;
        let to_rgb = |c: u8| if c == 0 { 0 } else { 55 + c * 40 };
        (to_rgb(r), to_rgb(g), to_rgb(b))
    } else {
        // Grayscale
        let gray = 8 + (n - 232) * 10;
        (gray, gray, gray)
    }
}

/// Linear interpolation for u8 values.
fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let a = a as f32;
    let b = b as f32;
    (a + (b - a) * t).round() as u8
}

/// A sequence of animations that play one after another.
///
/// # Example
///
/// ```rust
/// use inky::animation::{Animation, Sequence, Easing};
/// use std::time::Duration;
///
/// let seq = Sequence::new()
///     .then(Animation::new(0.0, 50.0, Duration::from_millis(200)))
///     .then(Animation::new(50.0, 100.0, Duration::from_millis(300)).easing(Easing::EaseOut));
/// ```
#[derive(Debug, Clone, Default)]
pub struct Sequence {
    /// Animations in order.
    animations: Vec<Animation>,
}

impl Sequence {
    /// Create a new empty sequence.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an animation to the sequence.
    pub fn then(mut self, animation: Animation) -> Self {
        self.animations.push(animation);
        self
    }

    /// Get the value at a specific elapsed time.
    pub fn value_at(&self, elapsed: Duration) -> f32 {
        if self.animations.is_empty() {
            return 0.0;
        }

        let mut remaining = elapsed;
        for anim in &self.animations {
            let anim_duration = anim.total_duration();
            if remaining < anim_duration {
                return anim.value_at(remaining);
            }
            remaining -= anim_duration;
        }

        // Past all animations, return final value
        self.animations.last().map(|a| a.to).unwrap_or(0.0)
    }

    /// Get total duration of the sequence.
    pub fn total_duration(&self) -> Duration {
        self.animations.iter().map(|a| a.total_duration()).sum()
    }

    /// Check if the sequence is complete.
    pub fn is_complete(&self, elapsed: Duration) -> bool {
        elapsed >= self.total_duration()
    }

    /// Get number of animations in the sequence.
    pub fn len(&self) -> usize {
        self.animations.len()
    }

    /// Check if the sequence is empty.
    pub fn is_empty(&self) -> bool {
        self.animations.is_empty()
    }
}

/// A group of animations that play simultaneously.
///
/// # Example
///
/// ```rust
/// use inky::animation::{Animation, Parallel};
/// use std::time::Duration;
///
/// let group = Parallel::new()
///     .add("x", Animation::new(0.0, 100.0, Duration::from_millis(500)))
///     .add("y", Animation::new(0.0, 50.0, Duration::from_millis(300)));
///
/// let values = group.values_at(Duration::from_millis(200));
/// // values["x"] and values["y"] contain the interpolated values
/// ```
#[derive(Debug, Clone, Default)]
pub struct Parallel {
    /// Named animations.
    animations: Vec<(String, Animation)>,
}

impl Parallel {
    /// Create a new empty parallel group.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a named animation to the group.
    pub fn add(mut self, name: impl Into<String>, animation: Animation) -> Self {
        self.animations.push((name.into(), animation));
        self
    }

    /// Get all values at a specific elapsed time.
    pub fn values_at(&self, elapsed: Duration) -> std::collections::HashMap<String, f32> {
        self.animations
            .iter()
            .map(|(name, anim)| (name.clone(), anim.value_at(elapsed)))
            .collect()
    }

    /// Get a single value by name at a specific elapsed time.
    pub fn value_at(&self, name: &str, elapsed: Duration) -> Option<f32> {
        self.animations
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, anim)| anim.value_at(elapsed))
    }

    /// Get total duration (longest animation).
    pub fn total_duration(&self) -> Duration {
        self.animations
            .iter()
            .map(|(_, a)| a.total_duration())
            .max()
            .unwrap_or(Duration::ZERO)
    }

    /// Check if all animations are complete.
    pub fn is_complete(&self, elapsed: Duration) -> bool {
        self.animations.iter().all(|(_, a)| a.is_complete(elapsed))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_easing() {
        assert_eq!(Easing::Linear.apply(0.0), 0.0);
        assert_eq!(Easing::Linear.apply(0.5), 0.5);
        assert_eq!(Easing::Linear.apply(1.0), 1.0);
    }

    #[test]
    fn test_ease_in() {
        let ease = Easing::EaseIn;
        assert_eq!(ease.apply(0.0), 0.0);
        assert_eq!(ease.apply(1.0), 1.0);
        // Ease in should be slower at start (value < time)
        assert!(ease.apply(0.5) < 0.5);
    }

    #[test]
    fn test_ease_out() {
        let ease = Easing::EaseOut;
        assert_eq!(ease.apply(0.0), 0.0);
        assert_eq!(ease.apply(1.0), 1.0);
        // Ease out should be faster at start (value > time)
        assert!(ease.apply(0.5) > 0.5);
    }

    #[test]
    fn test_animation_basic() {
        let anim = Animation::new(0.0, 100.0, Duration::from_millis(100));

        assert_eq!(anim.value_at(Duration::ZERO), 0.0);
        assert_eq!(anim.value_at(Duration::from_millis(50)), 50.0);
        assert_eq!(anim.value_at(Duration::from_millis(100)), 100.0);
        assert_eq!(anim.value_at(Duration::from_millis(200)), 100.0); // Clamped
    }

    #[test]
    fn test_animation_with_easing() {
        let anim = Animation::new(0.0, 100.0, Duration::from_millis(100)).easing(Easing::EaseIn);

        let mid_value = anim.value_at(Duration::from_millis(50));
        // With ease-in, midpoint should be less than 50
        assert!(mid_value < 50.0);
    }

    #[test]
    fn test_animation_with_delay() {
        let anim =
            Animation::new(0.0, 100.0, Duration::from_millis(100)).delay(Duration::from_millis(50));

        // During delay, should return start value
        assert_eq!(anim.value_at(Duration::ZERO), 0.0);
        assert_eq!(anim.value_at(Duration::from_millis(25)), 0.0);

        // After delay, animation progresses
        assert_eq!(anim.value_at(Duration::from_millis(50)), 0.0);
        assert_eq!(anim.value_at(Duration::from_millis(100)), 50.0);
        assert_eq!(anim.value_at(Duration::from_millis(150)), 100.0);
    }

    #[test]
    fn test_animation_is_complete() {
        let anim = Animation::new(0.0, 100.0, Duration::from_millis(100));

        assert!(!anim.is_complete(Duration::ZERO));
        assert!(!anim.is_complete(Duration::from_millis(50)));
        assert!(anim.is_complete(Duration::from_millis(100)));
        assert!(anim.is_complete(Duration::from_millis(200)));
    }

    #[test]
    fn test_repeating_animation() {
        let anim = Animation::new(0.0, 100.0, Duration::from_millis(100)).repeat();

        assert_eq!(anim.value_at(Duration::ZERO), 0.0);
        assert_eq!(anim.value_at(Duration::from_millis(50)), 50.0);
        assert_eq!(anim.value_at(Duration::from_millis(100)), 0.0); // Restart
        assert_eq!(anim.value_at(Duration::from_millis(150)), 50.0);
    }

    #[test]
    fn test_alternating_animation() {
        let anim = Animation::new(0.0, 100.0, Duration::from_millis(100))
            .repeat()
            .alternate();

        assert_eq!(anim.value_at(Duration::ZERO), 0.0);
        assert_eq!(anim.value_at(Duration::from_millis(100)), 100.0); // End of first cycle
        assert_eq!(anim.value_at(Duration::from_millis(150)), 50.0); // Going back
        assert_eq!(anim.value_at(Duration::from_millis(200)), 0.0); // Back to start
    }

    #[test]
    fn test_sequence() {
        let seq = Sequence::new()
            .then(Animation::new(0.0, 50.0, Duration::from_millis(100)))
            .then(Animation::new(50.0, 100.0, Duration::from_millis(100)));

        assert_eq!(seq.value_at(Duration::ZERO), 0.0);
        assert_eq!(seq.value_at(Duration::from_millis(50)), 25.0);
        assert_eq!(seq.value_at(Duration::from_millis(100)), 50.0);
        assert_eq!(seq.value_at(Duration::from_millis(150)), 75.0);
        assert_eq!(seq.value_at(Duration::from_millis(200)), 100.0);
    }

    #[test]
    fn test_parallel() {
        let group = Parallel::new()
            .add("x", Animation::new(0.0, 100.0, Duration::from_millis(100)))
            .add("y", Animation::new(0.0, 50.0, Duration::from_millis(100)));

        let values = group.values_at(Duration::from_millis(50));
        assert_eq!(values["x"], 50.0);
        assert_eq!(values["y"], 25.0);
    }

    #[test]
    fn test_lerp_color() {
        use crate::style::Color;

        let from = Color::Black;
        let to = Color::White;

        let mid = lerp_color(from, to, 0.5);
        assert!(matches!(mid, Color::Rgb(r, g, b) if (100..130).contains(&r)
            && (100..130).contains(&g)
            && (100..130).contains(&b)));
    }

    #[test]
    fn test_easing_clamps_input() {
        // Test that out-of-range inputs are clamped
        assert_eq!(Easing::Linear.apply(-0.5), 0.0);
        assert_eq!(Easing::Linear.apply(1.5), 1.0);
    }

    #[test]
    fn test_bounce_easing() {
        let ease = Easing::Bounce;
        assert_eq!(ease.apply(0.0), 0.0);
        assert!((ease.apply(1.0) - 1.0).abs() < 0.001);
        // Bounce should approach 1.0 asymptotically with bounces
        assert!(ease.apply(0.9) > 0.9);
    }

    #[test]
    fn test_elastic_easing() {
        let ease = Easing::Elastic;
        assert_eq!(ease.apply(0.0), 0.0);
        assert_eq!(ease.apply(1.0), 1.0);
        // Elastic can overshoot
    }
}
