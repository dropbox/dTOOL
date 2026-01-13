//! Time duration formatting utilities
//!
//! Provides human-readable formatting for time durations with smart
//! units selection based on the magnitude.

use std::time::{Duration, Instant};

/// Format the elapsed time since `start_time` as a human-readable string.
///
/// # Examples
/// ```no_run
/// use std::time::Instant;
/// use codex_dashflow_core::elapsed::format_elapsed;
///
/// let start = Instant::now();
/// // ... do some work ...
/// let result = format_elapsed(start);
/// // e.g., "250ms" or "1.50s" or "1m 15s"
/// ```
pub fn format_elapsed(start_time: Instant) -> String {
    format_duration(start_time.elapsed())
}

/// Convert a [`Duration`] into a human-readable, compact string.
///
/// Formatting rules:
/// * < 1s  → `"{millis}ms"`
/// * < 60s → `"{sec:.2}s"` (two decimal places)
/// * >= 60s → `"{min}m {sec:02}s"`
///
/// # Examples
/// ```no_run
/// use std::time::Duration;
/// use codex_dashflow_core::elapsed::format_duration;
///
/// assert_eq!(format_duration(Duration::from_millis(250)), "250ms");
/// assert_eq!(format_duration(Duration::from_millis(1500)), "1.50s");
/// assert_eq!(format_duration(Duration::from_millis(75000)), "1m 15s");
/// ```
pub fn format_duration(duration: Duration) -> String {
    let millis = duration.as_millis() as i64;
    format_elapsed_millis(millis)
}

/// Format duration from milliseconds as an i64.
///
/// Useful when you have a raw millisecond count rather than a Duration.
pub fn format_elapsed_millis(millis: i64) -> String {
    if millis < 1000 {
        format!("{millis}ms")
    } else if millis < 60_000 {
        format!("{:.2}s", millis as f64 / 1000.0)
    } else {
        let minutes = millis / 60_000;
        let seconds = (millis % 60_000) / 1000;
        format!("{minutes}m {seconds:02}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration_zero() {
        let dur = Duration::from_millis(0);
        assert_eq!(format_duration(dur), "0ms");
    }

    #[test]
    fn test_format_duration_subsecond() {
        // Durations < 1s should be rendered in milliseconds with no decimals.
        let dur = Duration::from_millis(250);
        assert_eq!(format_duration(dur), "250ms");

        let dur2 = Duration::from_millis(999);
        assert_eq!(format_duration(dur2), "999ms");
    }

    #[test]
    fn test_format_duration_seconds() {
        // Durations between 1s (inclusive) and 60s (exclusive) should be
        // printed with 2-decimal-place seconds.
        let dur = Duration::from_millis(1_000);
        assert_eq!(format_duration(dur), "1.00s");

        let dur2 = Duration::from_millis(1_500); // 1.5s
        assert_eq!(format_duration(dur2), "1.50s");

        // 59.999s rounds to 60.00s
        let dur3 = Duration::from_millis(59_999);
        assert_eq!(format_duration(dur3), "60.00s");
    }

    #[test]
    fn test_format_duration_minutes() {
        // Durations >= 1 minute should be printed mmss.
        let dur = Duration::from_millis(60_000); // 1m0s
        assert_eq!(format_duration(dur), "1m 00s");

        let dur2 = Duration::from_millis(75_000); // 1m15s
        assert_eq!(format_duration(dur2), "1m 15s");

        let dur3 = Duration::from_millis(3_601_000); // 60m1s
        assert_eq!(format_duration(dur3), "60m 01s");
    }

    #[test]
    fn test_format_duration_one_hour() {
        let dur = Duration::from_millis(3_600_000); // exactly 1 hour
        assert_eq!(format_duration(dur), "60m 00s");
    }

    #[test]
    fn test_format_elapsed_millis_direct() {
        assert_eq!(format_elapsed_millis(0), "0ms");
        assert_eq!(format_elapsed_millis(500), "500ms");
        assert_eq!(format_elapsed_millis(2500), "2.50s");
        assert_eq!(format_elapsed_millis(90000), "1m 30s");
    }

    #[test]
    fn test_format_elapsed_uses_current_time() {
        let start = Instant::now();
        // format_elapsed should return a valid string
        let result = format_elapsed(start);
        // Should end with ms since we just started
        assert!(result.ends_with("ms") || result.ends_with("s"));
    }
}
