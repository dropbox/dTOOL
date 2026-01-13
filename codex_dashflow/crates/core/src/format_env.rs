//! Environment variable display formatting
//!
//! Provides utilities for formatting environment variable names for display,
//! masking values for security while showing which variables are set.

use std::collections::HashMap;

/// Format environment variables for secure display.
///
/// Takes an optional map of environment variables and a list of additional
/// variable names, and returns a string representation with values masked.
/// The output is sorted alphabetically by variable name.
///
/// # Arguments
/// * `env` - Optional HashMap of environment variable names to values
/// * `env_vars` - Additional variable names to include
///
/// # Returns
/// A comma-separated string of `NAME=*****` pairs, or `"-"` if both inputs are empty.
///
/// # Examples
/// ```no_run
/// use std::collections::HashMap;
/// use codex_dashflow_core::format_env::format_env_display;
///
/// // No environment variables
/// assert_eq!(format_env_display(None, &[]), "-");
///
/// // With environment map
/// let mut env = HashMap::new();
/// env.insert("API_KEY".to_string(), "secret".to_string());
/// assert_eq!(format_env_display(Some(&env), &[]), "API_KEY=*****");
///
/// // With additional vars
/// assert_eq!(
///     format_env_display(None, &["TOKEN".to_string()]),
///     "TOKEN=*****"
/// );
/// ```
pub fn format_env_display(env: Option<&HashMap<String, String>>, env_vars: &[String]) -> String {
    let mut parts: Vec<String> = Vec::new();

    if let Some(map) = env {
        let mut pairs: Vec<_> = map.iter().collect();
        pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
        parts.extend(pairs.into_iter().map(|(key, _)| format!("{key}=*****")));
    }

    if !env_vars.is_empty() {
        parts.extend(env_vars.iter().map(|var| format!("{var}=*****")));
    }

    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.join(", ")
    }
}

/// Format a single environment variable for secure display.
///
/// # Examples
/// ```no_run
/// use codex_dashflow_core::format_env::format_env_var_masked;
///
/// assert_eq!(format_env_var_masked("API_KEY"), "API_KEY=*****");
/// ```
pub fn format_env_var_masked(name: &str) -> String {
    format!("{name}=*****")
}

/// Format environment variables as a sorted, masked list.
///
/// Convenience function when you only have a HashMap.
///
/// # Examples
/// ```no_run
/// use std::collections::HashMap;
/// use codex_dashflow_core::format_env::format_env_map_display;
///
/// let mut env = HashMap::new();
/// env.insert("B".to_string(), "two".to_string());
/// env.insert("A".to_string(), "one".to_string());
///
/// assert_eq!(format_env_map_display(&env), "A=*****, B=*****");
/// ```
pub fn format_env_map_display(env: &HashMap<String, String>) -> String {
    if env.is_empty() {
        return "-".to_string();
    }

    let mut pairs: Vec<_> = env.keys().collect();
    pairs.sort();
    pairs
        .into_iter()
        .map(|key| format!("{key}=*****"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_returns_dash_when_empty() {
        assert_eq!(format_env_display(None, &[]), "-");

        let empty_map = HashMap::new();
        assert_eq!(format_env_display(Some(&empty_map), &[]), "-");
    }

    #[test]
    fn test_formats_sorted_env_pairs() {
        let mut env = HashMap::new();
        env.insert("B".to_string(), "two".to_string());
        env.insert("A".to_string(), "one".to_string());

        assert_eq!(format_env_display(Some(&env), &[]), "A=*****, B=*****");
    }

    #[test]
    fn test_formats_env_vars() {
        let vars = vec!["TOKEN".to_string(), "PATH".to_string()];

        assert_eq!(format_env_display(None, &vars), "TOKEN=*****, PATH=*****");
    }

    #[test]
    fn test_combines_env_pairs_and_vars() {
        let mut env = HashMap::new();
        env.insert("HOME".to_string(), "/tmp".to_string());
        let vars = vec!["TOKEN".to_string()];

        assert_eq!(
            format_env_display(Some(&env), &vars),
            "HOME=*****, TOKEN=*****"
        );
    }

    #[test]
    fn test_format_env_var_masked() {
        assert_eq!(format_env_var_masked("API_KEY"), "API_KEY=*****");
        assert_eq!(format_env_var_masked("SECRET"), "SECRET=*****");
    }

    #[test]
    fn test_format_env_map_display_empty() {
        let env = HashMap::new();
        assert_eq!(format_env_map_display(&env), "-");
    }

    #[test]
    fn test_format_env_map_display_sorted() {
        let mut env = HashMap::new();
        env.insert("ZEBRA".to_string(), "z".to_string());
        env.insert("ALPHA".to_string(), "a".to_string());
        env.insert("BETA".to_string(), "b".to_string());

        assert_eq!(
            format_env_map_display(&env),
            "ALPHA=*****, BETA=*****, ZEBRA=*****"
        );
    }
}
