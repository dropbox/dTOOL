//! Common utility functions
//!
//! This module provides utility functions used across the codebase:
//! - Retry backoff calculation with jitter
//! - Error message parsing from JSON responses
//! - Path resolution helpers

use std::path::{Path, PathBuf};
use std::time::Duration;

use rand::Rng;
use tracing::{debug, error};

/// Initial delay for retry backoff in milliseconds.
const INITIAL_DELAY_MS: u64 = 200;

/// Exponential backoff factor.
const BACKOFF_FACTOR: f64 = 2.0;

/// Calculate backoff duration for retry attempt.
///
/// Uses exponential backoff with jitter to prevent thundering herd.
/// First attempt (0) has no delay, subsequent attempts increase exponentially.
pub fn backoff(attempt: u64) -> Duration {
    if attempt == 0 {
        return Duration::ZERO;
    }
    let exp = BACKOFF_FACTOR.powi(attempt.saturating_sub(1) as i32);
    let base = (INITIAL_DELAY_MS as f64 * exp) as u64;
    let jitter = rand::rng().random_range(0.9..1.1);
    Duration::from_millis((base as f64 * jitter) as u64)
}

/// Log an error, or panic in debug builds.
///
/// In debug/alpha builds, this panics to catch issues early.
/// In release builds, it logs an error and continues.
pub fn error_or_panic(message: impl std::string::ToString) {
    if cfg!(debug_assertions) || env!("CARGO_PKG_VERSION").contains("alpha") {
        panic!("{}", message.to_string());
    } else {
        error!("{}", message.to_string());
    }
}

/// Try to parse an error message from a JSON response.
///
/// Looks for `error.message` field in the JSON, falls back to the raw text.
pub fn try_parse_error_message(text: &str) -> String {
    debug!("Parsing server error response: {}", text);
    let json = serde_json::from_str::<serde_json::Value>(text).unwrap_or_default();
    if let Some(error) = json.get("error") {
        if let Some(message) = error.get("message") {
            if let Some(message_str) = message.as_str() {
                return message_str.to_string();
            }
        }
    }
    if text.is_empty() {
        return "Unknown error".to_string();
    }
    text.to_string()
}

/// Resolve a path relative to a base directory.
///
/// If the path is absolute, returns it unchanged.
/// If relative, joins it to the base directory.
pub fn resolve_path(base: &Path, path: &PathBuf) -> PathBuf {
    if path.is_absolute() {
        path.clone()
    } else {
        base.join(path)
    }
}

/// Truncate a string to a maximum length, adding ellipsis if needed.
pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        "...".to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

/// Check if a string looks like a valid JSON object or array.
pub fn is_json_like(s: &str) -> bool {
    let trimmed = s.trim();
    (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backoff_zero() {
        let d = backoff(0);
        assert_eq!(d, Duration::ZERO);
    }

    #[test]
    fn test_backoff_increases() {
        let d1 = backoff(1);
        let d2 = backoff(2);
        let d3 = backoff(3);

        // Should increase (with some tolerance for jitter)
        assert!(d1.as_millis() > 0);
        assert!(d2.as_millis() >= d1.as_millis() / 2); // Allow jitter
        assert!(d3.as_millis() >= d2.as_millis() / 2);
    }

    #[test]
    fn test_try_parse_error_message_with_error() {
        let text = r#"{
  "error": {
    "message": "Your refresh token has already been used.",
    "type": "invalid_request_error",
    "code": "refresh_token_reused"
  }
}"#;
        let message = try_parse_error_message(text);
        assert_eq!(message, "Your refresh token has already been used.");
    }

    #[test]
    fn test_try_parse_error_message_no_error() {
        let text = r#"{"message": "test"}"#;
        let message = try_parse_error_message(text);
        assert_eq!(message, r#"{"message": "test"}"#);
    }

    #[test]
    fn test_try_parse_error_message_empty() {
        let message = try_parse_error_message("");
        assert_eq!(message, "Unknown error");
    }

    #[test]
    fn test_try_parse_error_message_plain_text() {
        let text = "Internal Server Error";
        let message = try_parse_error_message(text);
        assert_eq!(message, "Internal Server Error");
    }

    #[test]
    fn test_resolve_path_absolute() {
        let base = Path::new("/home/user");
        let path = PathBuf::from("/tmp/file.txt");
        let resolved = resolve_path(base, &path);
        assert_eq!(resolved, PathBuf::from("/tmp/file.txt"));
    }

    #[test]
    fn test_resolve_path_relative() {
        let base = Path::new("/home/user");
        let path = PathBuf::from("documents/file.txt");
        let resolved = resolve_path(base, &path);
        assert_eq!(resolved, PathBuf::from("/home/user/documents/file.txt"));
    }

    #[test]
    fn test_truncate_string_short() {
        assert_eq!(truncate_string("hello", 10), "hello");
        assert_eq!(truncate_string("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_string_long() {
        assert_eq!(truncate_string("hello world", 8), "hello...");
        assert_eq!(truncate_string("abcdefghij", 6), "abc...");
    }

    #[test]
    fn test_truncate_string_very_short_max() {
        assert_eq!(truncate_string("hello", 3), "...");
        assert_eq!(truncate_string("hello", 2), "...");
    }

    #[test]
    fn test_is_json_like() {
        assert!(is_json_like(r#"{"key": "value"}"#));
        assert!(is_json_like(r#"[1, 2, 3]"#));
        assert!(is_json_like("  { }  "));
        assert!(!is_json_like("hello"));
        assert!(!is_json_like("{incomplete"));
        assert!(!is_json_like(""));
    }
}
