//! Configuration override module.
//!
//! Provides utilities to parse and apply configuration overrides specified
//! as `key=value` pairs (commonly from CLI arguments like `-c key=value`).
//!
//! Values are parsed as TOML, with fallback to string literals for convenience.

use serde::de::Error as SerdeError;
use toml::Value;

/// Parses a raw override string into a (path, value) tuple.
///
/// The format is `key=value` where:
/// - `key` can be a dotted path (e.g., `sandbox.network_access`)
/// - `value` is parsed as TOML; if parsing fails, treated as a raw string
///
/// # Arguments
///
/// * `raw` - The raw override string (e.g., "model=gpt-4" or "sandbox_permissions=[\"read\"]")
///
/// # Returns
///
/// A tuple of (path, value) where value is a TOML Value, or an error.
///
/// # Example
///
/// ```no_run
/// use codex_dashflow_core::config_override::parse_override;
///
/// let (key, value) = parse_override("model=gpt-4").unwrap();
/// assert_eq!(key, "model");
/// assert_eq!(value.as_str(), Some("gpt-4"));
///
/// let (key, value) = parse_override("max_turns=10").unwrap();
/// assert_eq!(key, "max_turns");
/// assert_eq!(value.as_integer(), Some(10));
/// ```
pub fn parse_override(raw: &str) -> Result<(String, Value), String> {
    // Only split on the *first* '=' so values are free to contain the character
    let mut parts = raw.splitn(2, '=');
    let key = match parts.next() {
        Some(k) => k.trim(),
        None => return Err("Override missing key".to_string()),
    };
    let value_str = parts
        .next()
        .ok_or_else(|| format!("Invalid override (missing '='): {}", raw))?
        .trim();

    if key.is_empty() {
        return Err(format!("Empty key in override: {}", raw));
    }

    // Attempt to parse as TOML. If that fails, treat it as a raw string.
    let value: Value = match parse_toml_value(value_str) {
        Ok(v) => v,
        Err(_) => {
            // Strip leading/trailing quotes if present
            let trimmed = value_str.trim().trim_matches(|c| c == '"' || c == '\'');
            Value::String(trimmed.to_string())
        }
    };

    Ok((key.to_string(), value))
}

/// Parses multiple override strings into a list of (path, value) tuples.
///
/// # Arguments
///
/// * `overrides` - Iterator of raw override strings
///
/// # Returns
///
/// A vector of (path, value) tuples, or the first error encountered.
///
/// # Example
///
/// ```no_run
/// use codex_dashflow_core::config_override::parse_overrides;
///
/// let raw = vec!["model=gpt-4", "max_turns=10"];
/// let parsed = parse_overrides(raw.iter().map(|s| s.to_string())).unwrap();
/// assert_eq!(parsed.len(), 2);
/// ```
pub fn parse_overrides(
    overrides: impl IntoIterator<Item = String>,
) -> Result<Vec<(String, Value)>, String> {
    overrides.into_iter().map(|s| parse_override(&s)).collect()
}

/// Applies a single override onto a TOML value, creating intermediate tables as needed.
///
/// # Arguments
///
/// * `root` - The root TOML value to modify
/// * `path` - Dotted path to the target key (e.g., "sandbox.network_access")
/// * `value` - The value to set
///
/// # Example
///
/// ```no_run
/// use codex_dashflow_core::config_override::apply_single_override;
/// use toml::Value;
///
/// let mut root = Value::Table(toml::map::Map::new());
/// apply_single_override(&mut root, "model", Value::String("gpt-4".into()));
/// assert_eq!(root.get("model").and_then(|v| v.as_str()), Some("gpt-4"));
///
/// // Nested paths create intermediate tables
/// apply_single_override(&mut root, "sandbox.network", Value::Boolean(true));
/// assert!(root.get("sandbox").is_some());
/// ```
pub fn apply_single_override(root: &mut Value, path: &str, value: Value) {
    use toml::map::Map;

    let parts: Vec<&str> = path.split('.').collect();
    let mut current = root;

    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;

        if is_last {
            match current {
                Value::Table(tbl) => {
                    tbl.insert((*part).to_string(), value);
                }
                _ => {
                    let mut tbl = Map::new();
                    tbl.insert((*part).to_string(), value);
                    *current = Value::Table(tbl);
                }
            }
            return;
        }

        // Traverse or create intermediate table
        match current {
            Value::Table(tbl) => {
                current = tbl
                    .entry((*part).to_string())
                    .or_insert_with(|| Value::Table(Map::new()));
            }
            _ => {
                *current = Value::Table(Map::new());
                if let Value::Table(tbl) = current {
                    current = tbl
                        .entry((*part).to_string())
                        .or_insert_with(|| Value::Table(Map::new()));
                }
            }
        }
    }
}

/// Applies multiple overrides onto a TOML value.
///
/// # Arguments
///
/// * `root` - The root TOML value to modify
/// * `overrides` - List of (path, value) tuples from `parse_overrides`
///
/// # Example
///
/// ```no_run
/// use codex_dashflow_core::config_override::{apply_overrides, parse_overrides};
/// use toml::Value;
///
/// let mut root = Value::Table(toml::map::Map::new());
/// let overrides = parse_overrides(vec!["model=gpt-4".into(), "max_turns=5".into()]).unwrap();
/// apply_overrides(&mut root, &overrides);
///
/// assert_eq!(root.get("model").and_then(|v| v.as_str()), Some("gpt-4"));
/// assert_eq!(root.get("max_turns").and_then(|v| v.as_integer()), Some(5));
/// ```
pub fn apply_overrides(root: &mut Value, overrides: &[(String, Value)]) {
    for (path, value) in overrides {
        apply_single_override(root, path, value.clone());
    }
}

/// Parses and applies override strings onto a TOML value.
///
/// Convenience function combining `parse_overrides` and `apply_overrides`.
///
/// # Arguments
///
/// * `root` - The root TOML value to modify
/// * `raw_overrides` - Iterator of raw override strings
///
/// # Returns
///
/// `Ok(())` on success, or the first parsing error.
pub fn parse_and_apply_overrides(
    root: &mut Value,
    raw_overrides: impl IntoIterator<Item = String>,
) -> Result<(), String> {
    let overrides = parse_overrides(raw_overrides)?;
    apply_overrides(root, &overrides);
    Ok(())
}

/// Parses a single value as TOML.
///
/// Used internally to parse the right-hand side of `key=value` overrides.
fn parse_toml_value(raw: &str) -> Result<Value, toml::de::Error> {
    // Wrap the value in a sentinel key so we can parse arbitrary values
    let wrapped = format!("_x_ = {}", raw);
    let table: toml::Table = toml::from_str(&wrapped)?;
    table
        .get("_x_")
        .cloned()
        .ok_or_else(|| SerdeError::custom("missing sentinel key"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_override_string() {
        let (key, value) = parse_override("model=gpt-4").unwrap();
        assert_eq!(key, "model");
        assert_eq!(value.as_str(), Some("gpt-4"));
    }

    #[test]
    fn test_parse_override_integer() {
        let (key, value) = parse_override("max_turns=42").unwrap();
        assert_eq!(key, "max_turns");
        assert_eq!(value.as_integer(), Some(42));
    }

    #[test]
    fn test_parse_override_bool_true() {
        let (key, value) = parse_override("enabled=true").unwrap();
        assert_eq!(key, "enabled");
        assert_eq!(value.as_bool(), Some(true));
    }

    #[test]
    fn test_parse_override_bool_false() {
        let (key, value) = parse_override("enabled=false").unwrap();
        assert_eq!(key, "enabled");
        assert_eq!(value.as_bool(), Some(false));
    }

    #[test]
    fn test_parse_override_array() {
        let (key, value) = parse_override(r#"items=[1, 2, 3]"#).unwrap();
        assert_eq!(key, "items");
        let arr = value.as_array().unwrap();
        assert_eq!(arr.len(), 3);
    }

    #[test]
    fn test_parse_override_inline_table() {
        let (key, value) = parse_override(r#"config={a = 1, b = 2}"#).unwrap();
        assert_eq!(key, "config");
        let tbl = value.as_table().unwrap();
        assert_eq!(tbl.get("a").unwrap().as_integer(), Some(1));
        assert_eq!(tbl.get("b").unwrap().as_integer(), Some(2));
    }

    #[test]
    fn test_parse_override_quoted_string() {
        // Quoted strings should work as TOML
        let (key, value) = parse_override(r#"model="gpt-4""#).unwrap();
        assert_eq!(key, "model");
        assert_eq!(value.as_str(), Some("gpt-4"));
    }

    #[test]
    fn test_parse_override_unquoted_fallback() {
        // Unquoted strings that don't parse as TOML fall back to string
        let (key, value) = parse_override("model=my-model-name").unwrap();
        assert_eq!(key, "model");
        assert_eq!(value.as_str(), Some("my-model-name"));
    }

    #[test]
    fn test_parse_override_value_with_equals() {
        // Value containing '=' should work
        let (key, value) = parse_override("header=Authorization=Bearer token").unwrap();
        assert_eq!(key, "header");
        assert_eq!(value.as_str(), Some("Authorization=Bearer token"));
    }

    #[test]
    fn test_parse_override_dotted_key() {
        let (key, value) = parse_override("sandbox.network=true").unwrap();
        assert_eq!(key, "sandbox.network");
        assert_eq!(value.as_bool(), Some(true));
    }

    #[test]
    fn test_parse_override_missing_equals() {
        let result = parse_override("noequals");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing '='"));
    }

    #[test]
    fn test_parse_override_empty_key() {
        let result = parse_override("=value");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Empty key"));
    }

    #[test]
    fn test_parse_overrides_multiple() {
        let raw = vec![
            "a=1".to_string(),
            "b=hello".to_string(),
            "c=true".to_string(),
        ];
        let parsed = parse_overrides(raw).unwrap();
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].0, "a");
        assert_eq!(parsed[1].0, "b");
        assert_eq!(parsed[2].0, "c");
    }

    #[test]
    fn test_apply_single_override_simple() {
        let mut root = Value::Table(toml::map::Map::new());
        apply_single_override(&mut root, "model", Value::String("gpt-4".into()));

        assert_eq!(root.get("model").and_then(|v| v.as_str()), Some("gpt-4"));
    }

    #[test]
    fn test_apply_single_override_nested() {
        let mut root = Value::Table(toml::map::Map::new());
        apply_single_override(&mut root, "sandbox.network", Value::Boolean(true));

        let sandbox = root.get("sandbox").and_then(|v| v.as_table()).unwrap();
        assert_eq!(sandbox.get("network").and_then(|v| v.as_bool()), Some(true));
    }

    #[test]
    fn test_apply_single_override_deeply_nested() {
        let mut root = Value::Table(toml::map::Map::new());
        apply_single_override(&mut root, "a.b.c.d", Value::Integer(42));

        let a = root.get("a").and_then(|v| v.as_table()).unwrap();
        let b = a.get("b").and_then(|v| v.as_table()).unwrap();
        let c = b.get("c").and_then(|v| v.as_table()).unwrap();
        assert_eq!(c.get("d").and_then(|v| v.as_integer()), Some(42));
    }

    #[test]
    fn test_apply_overrides_multiple() {
        let mut root = Value::Table(toml::map::Map::new());
        let overrides = vec![
            ("model".to_string(), Value::String("gpt-4".into())),
            ("max_turns".to_string(), Value::Integer(10)),
        ];
        apply_overrides(&mut root, &overrides);

        assert_eq!(root.get("model").and_then(|v| v.as_str()), Some("gpt-4"));
        assert_eq!(root.get("max_turns").and_then(|v| v.as_integer()), Some(10));
    }

    #[test]
    fn test_parse_and_apply_overrides() {
        let mut root = Value::Table(toml::map::Map::new());
        parse_and_apply_overrides(&mut root, vec!["model=gpt-4".into(), "enabled=true".into()])
            .unwrap();

        assert_eq!(root.get("model").and_then(|v| v.as_str()), Some("gpt-4"));
        assert_eq!(root.get("enabled").and_then(|v| v.as_bool()), Some(true));
    }

    #[test]
    fn test_override_replaces_existing() {
        let mut root = Value::Table(toml::map::Map::new());
        apply_single_override(&mut root, "model", Value::String("old".into()));
        apply_single_override(&mut root, "model", Value::String("new".into()));

        assert_eq!(root.get("model").and_then(|v| v.as_str()), Some("new"));
    }

    #[test]
    fn test_toml_value_float() {
        let (key, value) = parse_override("temperature=0.7").unwrap();
        assert_eq!(key, "temperature");
        // Note: TOML parses 0.7 as float
        assert!(value.as_float().is_some());
    }
}
