//! Terminal detection for user-agent headers.
//!
//! Detects the terminal emulator in use and provides a sanitized string
//! suitable for User-Agent headers in API requests.

use std::sync::OnceLock;

static TERMINAL: OnceLock<String> = OnceLock::new();

/// Returns a sanitized user-agent string identifying the terminal emulator.
///
/// The result is cached after the first call since terminal detection
/// reads environment variables that don't change during execution.
///
/// # Examples
///
/// ```no_run
/// let ua = codex_dashflow_core::terminal::user_agent();
/// // Returns something like "iTerm.app/3.5.0" or "Alacritty"
/// ```
pub fn user_agent() -> String {
    TERMINAL.get_or_init(detect_terminal).to_string()
}

/// Check if a character is valid for HTTP header values.
///
/// User-Agent headers should only contain ASCII alphanumeric characters
/// plus a few safe special characters.
fn is_valid_header_value_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/'
}

/// Sanitize a string to be safe for use in User-Agent headers.
///
/// Replaces any invalid characters with underscores.
fn sanitize_header_value(value: String) -> String {
    value.replace(|c| !is_valid_header_value_char(c), "_")
}

/// Detect the terminal emulator from environment variables.
///
/// Checks various terminal-specific environment variables to identify
/// the terminal in use. Falls back to $TERM if no specific terminal
/// is detected.
fn detect_terminal() -> String {
    sanitize_header_value(detect_terminal_raw())
}

fn detect_terminal_raw() -> String {
    // Check TERM_PROGRAM first (used by many terminals including iTerm2, Apple Terminal)
    if let Ok(tp) = std::env::var("TERM_PROGRAM") {
        if !tp.trim().is_empty() {
            let ver = std::env::var("TERM_PROGRAM_VERSION").ok();
            return match ver {
                Some(v) if !v.trim().is_empty() => format!("{tp}/{v}"),
                _ => tp,
            };
        }
    }

    // WezTerm
    if let Ok(v) = std::env::var("WEZTERM_VERSION") {
        if !v.trim().is_empty() {
            return format!("WezTerm/{v}");
        } else {
            return "WezTerm".to_string();
        }
    }

    // Kitty
    if std::env::var("KITTY_WINDOW_ID").is_ok()
        || std::env::var("TERM")
            .map(|t| t.contains("kitty"))
            .unwrap_or(false)
    {
        return "kitty".to_string();
    }

    // Alacritty
    if std::env::var("ALACRITTY_SOCKET").is_ok()
        || std::env::var("TERM")
            .map(|t| t == "alacritty")
            .unwrap_or(false)
    {
        return "Alacritty".to_string();
    }

    // Konsole
    if let Ok(v) = std::env::var("KONSOLE_VERSION") {
        if !v.trim().is_empty() {
            return format!("Konsole/{v}");
        } else {
            return "Konsole".to_string();
        }
    }

    // GNOME Terminal
    if std::env::var("GNOME_TERMINAL_SCREEN").is_ok() {
        return "gnome-terminal".to_string();
    }

    // VTE-based terminals
    if let Ok(v) = std::env::var("VTE_VERSION") {
        if !v.trim().is_empty() {
            return format!("VTE/{v}");
        } else {
            return "VTE".to_string();
        }
    }

    // Windows Terminal
    if std::env::var("WT_SESSION").is_ok() {
        return "WindowsTerminal".to_string();
    }

    // Fallback to $TERM
    std::env::var("TERM").unwrap_or_else(|_| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_header_value_alphanumeric() {
        assert_eq!(sanitize_header_value("iTerm".to_string()), "iTerm");
    }

    #[test]
    fn test_sanitize_header_value_with_version() {
        assert_eq!(
            sanitize_header_value("iTerm.app/3.5.0".to_string()),
            "iTerm.app/3.5.0"
        );
    }

    #[test]
    fn test_sanitize_header_value_with_spaces() {
        assert_eq!(
            sanitize_header_value("Apple Terminal".to_string()),
            "Apple_Terminal"
        );
    }

    #[test]
    fn test_sanitize_header_value_with_special_chars() {
        assert_eq!(
            sanitize_header_value("Term@1.0:beta".to_string()),
            "Term_1.0_beta"
        );
    }

    #[test]
    fn test_valid_header_chars() {
        assert!(is_valid_header_value_char('a'));
        assert!(is_valid_header_value_char('Z'));
        assert!(is_valid_header_value_char('0'));
        assert!(is_valid_header_value_char('-'));
        assert!(is_valid_header_value_char('_'));
        assert!(is_valid_header_value_char('.'));
        assert!(is_valid_header_value_char('/'));
    }

    #[test]
    fn test_invalid_header_chars() {
        assert!(!is_valid_header_value_char(' '));
        assert!(!is_valid_header_value_char('@'));
        assert!(!is_valid_header_value_char(':'));
        assert!(!is_valid_header_value_char('\n'));
    }

    #[test]
    fn test_user_agent_returns_string() {
        // Just verify it returns something without panicking
        // Actual value depends on environment
        let ua = user_agent();
        assert!(!ua.is_empty());
    }

    #[test]
    fn test_user_agent_is_cached() {
        // Calling twice should return the same cached value
        let ua1 = user_agent();
        let ua2 = user_agent();
        assert_eq!(ua1, ua2);
    }

    #[test]
    fn test_detect_terminal_raw_returns_string() {
        // Should return something, even if just "unknown"
        let result = detect_terminal_raw();
        assert!(!result.is_empty());
    }
}
