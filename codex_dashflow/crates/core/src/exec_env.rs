//! Environment variable filtering for shell execution
//!
//! This module provides functions to construct safe environment variable maps
//! for spawned processes. It filters out sensitive variables (like API keys and
//! secrets) while allowing configurable inheritance and exclusion policies.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;

/// Pattern for matching environment variable names
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentVariablePattern {
    /// The pattern string (supports * as wildcard)
    pattern: String,
    /// Whether to use case-insensitive matching
    case_insensitive: bool,
}

impl EnvironmentVariablePattern {
    /// Create a new case-sensitive pattern
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            case_insensitive: false,
        }
    }

    /// Create a new case-insensitive pattern
    pub fn new_case_insensitive(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            case_insensitive: true,
        }
    }

    /// Check if a name matches this pattern
    pub fn matches(&self, name: &str) -> bool {
        let pattern = if self.case_insensitive {
            self.pattern.to_lowercase()
        } else {
            self.pattern.clone()
        };
        let name = if self.case_insensitive {
            name.to_lowercase()
        } else {
            name.to_string()
        };

        // Simple glob matching with * as wildcard
        if pattern == "*" {
            return true;
        }

        // If no wildcards, require exact match
        if !pattern.contains('*') {
            return pattern == name;
        }

        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.is_empty() {
            return pattern == name;
        }

        let mut pos = 0;
        for (i, part) in parts.iter().enumerate() {
            if part.is_empty() {
                continue;
            }

            if i == 0 {
                // First part must match at start
                if !name.starts_with(*part) {
                    return false;
                }
                pos = part.len();
            } else if i == parts.len() - 1 {
                // Last part must match at end
                if !name.ends_with(*part) {
                    return false;
                }
            } else {
                // Middle parts must exist somewhere after pos
                if let Some(idx) = name[pos..].find(*part) {
                    pos = pos + idx + part.len();
                } else {
                    return false;
                }
            }
        }

        true
    }
}

/// Policy for how to inherit environment variables from the parent process
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShellEnvironmentPolicyInherit {
    /// Inherit all environment variables
    All,
    /// Inherit no environment variables (start empty)
    None,
    /// Inherit only core variables (PATH, HOME, USER, etc.)
    #[default]
    Core,
}

/// Policy for constructing shell environment variables
///
/// The derivation follows this algorithm:
/// 1. Start with variables based on `inherit` strategy
/// 2. Apply default excludes (unless `ignore_default_excludes` is true)
/// 3. Apply custom `exclude` patterns
/// 4. Apply `set` overrides
/// 5. If `include_only` is non-empty, keep only matching variables
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ShellEnvironmentPolicy {
    /// How to inherit environment variables from parent
    #[serde(default)]
    pub inherit: ShellEnvironmentPolicyInherit,

    /// Variables to exclude (supports glob patterns)
    #[serde(default)]
    pub exclude: Vec<EnvironmentVariablePattern>,

    /// If true, skip the default excludes (*KEY*, *SECRET*, *TOKEN*)
    #[serde(default)]
    pub ignore_default_excludes: bool,

    /// Variables to explicitly set (overrides any inherited value)
    #[serde(default)]
    pub set: HashMap<String, String>,

    /// If non-empty, keep only variables matching these patterns
    #[serde(default)]
    pub include_only: Vec<EnvironmentVariablePattern>,
}

/// Construct an environment map based on the rules in the specified policy.
///
/// The resulting map can be passed directly to `Command::envs()` after calling
/// `env_clear()` to ensure no unintended variables are leaked to the spawned
/// process.
pub fn create_env(policy: &ShellEnvironmentPolicy) -> HashMap<String, String> {
    populate_env(std::env::vars(), policy)
}

/// Internal function that accepts an iterator of variables for testability
fn populate_env<I>(vars: I, policy: &ShellEnvironmentPolicy) -> HashMap<String, String>
where
    I: IntoIterator<Item = (String, String)>,
{
    // Step 1 – determine the starting set of variables based on the
    // `inherit` strategy.
    let mut env_map: HashMap<String, String> = match policy.inherit {
        ShellEnvironmentPolicyInherit::All => vars.into_iter().collect(),
        ShellEnvironmentPolicyInherit::None => HashMap::new(),
        ShellEnvironmentPolicyInherit::Core => {
            const CORE_VARS: &[&str] = &[
                "HOME", "LOGNAME", "PATH", "SHELL", "USER", "USERNAME", "TMPDIR", "TEMP", "TMP",
            ];
            let allow: HashSet<&str> = CORE_VARS.iter().copied().collect();
            vars.into_iter()
                .filter(|(k, _)| allow.contains(k.as_str()))
                .collect()
        }
    };

    // Internal helper – does `name` match **any** pattern in `patterns`?
    let matches_any = |name: &str, patterns: &[EnvironmentVariablePattern]| -> bool {
        patterns.iter().any(|pattern| pattern.matches(name))
    };

    // Step 2 – Apply the default exclude if not disabled.
    if !policy.ignore_default_excludes {
        let default_excludes = vec![
            EnvironmentVariablePattern::new_case_insensitive("*KEY*"),
            EnvironmentVariablePattern::new_case_insensitive("*SECRET*"),
            EnvironmentVariablePattern::new_case_insensitive("*TOKEN*"),
        ];
        env_map.retain(|k, _| !matches_any(k, &default_excludes));
    }

    // Step 3 – Apply custom excludes.
    if !policy.exclude.is_empty() {
        env_map.retain(|k, _| !matches_any(k, &policy.exclude));
    }

    // Step 4 – Apply user-provided overrides.
    for (key, val) in &policy.set {
        env_map.insert(key.clone(), val.clone());
    }

    // Step 5 – If include_only is non-empty, keep *only* the matching vars.
    if !policy.include_only.is_empty() {
        env_map.retain(|k, _| matches_any(k, &policy.include_only));
    }

    env_map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_vars(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn test_pattern_exact_match() {
        let pattern = EnvironmentVariablePattern::new("PATH");
        assert!(pattern.matches("PATH"));
        assert!(!pattern.matches("path"));
        assert!(!pattern.matches("PATH2"));
    }

    #[test]
    fn test_pattern_case_insensitive() {
        let pattern = EnvironmentVariablePattern::new_case_insensitive("*key*");
        assert!(pattern.matches("API_KEY"));
        assert!(pattern.matches("api_key"));
        assert!(pattern.matches("MyKeyValue"));
        assert!(!pattern.matches("PASSWORD"));
    }

    #[test]
    fn test_pattern_prefix_wildcard() {
        let pattern = EnvironmentVariablePattern::new("*PATH");
        assert!(pattern.matches("PATH"));
        assert!(pattern.matches("MYPATH"));
        assert!(pattern.matches("MY_PATH"));
        assert!(!pattern.matches("PATHVAR"));
    }

    #[test]
    fn test_pattern_suffix_wildcard() {
        let pattern = EnvironmentVariablePattern::new("PATH*");
        assert!(pattern.matches("PATH"));
        assert!(pattern.matches("PATHVAR"));
        assert!(pattern.matches("PATH_EXTRA"));
        assert!(!pattern.matches("MYPATH"));
    }

    #[test]
    fn test_pattern_both_wildcards() {
        let pattern = EnvironmentVariablePattern::new("*KEY*");
        assert!(pattern.matches("API_KEY"));
        assert!(pattern.matches("KEY"));
        assert!(pattern.matches("KEYRING"));
        assert!(pattern.matches("MY_KEY_VAR"));
        assert!(!pattern.matches("PASSWORD"));
    }

    #[test]
    fn test_pattern_star_only() {
        let pattern = EnvironmentVariablePattern::new("*");
        assert!(pattern.matches("ANYTHING"));
        assert!(pattern.matches(""));
    }

    #[test]
    fn test_core_inherit_and_default_excludes() {
        let vars = make_vars(&[
            ("PATH", "/usr/bin"),
            ("HOME", "/home/user"),
            ("API_KEY", "secret"),
            ("SECRET_TOKEN", "t"),
        ]);

        let policy = ShellEnvironmentPolicy::default(); // inherit Core, default excludes on
        let result = populate_env(vars, &policy);

        assert_eq!(result.get("PATH"), Some(&"/usr/bin".to_string()));
        assert_eq!(result.get("HOME"), Some(&"/home/user".to_string()));
        assert!(!result.contains_key("API_KEY"));
        assert!(!result.contains_key("SECRET_TOKEN"));
    }

    #[test]
    fn test_include_only() {
        let vars = make_vars(&[("PATH", "/usr/bin"), ("FOO", "bar")]);

        let policy = ShellEnvironmentPolicy {
            // skip default excludes so nothing is removed prematurely
            ignore_default_excludes: true,
            include_only: vec![EnvironmentVariablePattern::new_case_insensitive("*PATH")],
            ..Default::default()
        };

        let result = populate_env(vars, &policy);

        assert_eq!(result.get("PATH"), Some(&"/usr/bin".to_string()));
        assert!(!result.contains_key("FOO"));
    }

    #[test]
    fn test_set_overrides() {
        let vars = make_vars(&[("PATH", "/usr/bin")]);

        let mut policy = ShellEnvironmentPolicy {
            ignore_default_excludes: true,
            ..Default::default()
        };
        policy.set.insert("NEW_VAR".to_string(), "42".to_string());

        let result = populate_env(vars, &policy);

        assert_eq!(result.get("PATH"), Some(&"/usr/bin".to_string()));
        assert_eq!(result.get("NEW_VAR"), Some(&"42".to_string()));
    }

    #[test]
    fn test_inherit_all() {
        let vars = make_vars(&[("PATH", "/usr/bin"), ("FOO", "bar")]);

        let policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::All,
            ignore_default_excludes: true, // keep everything
            ..Default::default()
        };

        let result = populate_env(vars.clone(), &policy);
        assert_eq!(result.get("PATH"), Some(&"/usr/bin".to_string()));
        assert_eq!(result.get("FOO"), Some(&"bar".to_string()));
    }

    #[test]
    fn test_inherit_all_with_default_excludes() {
        let vars = make_vars(&[("PATH", "/usr/bin"), ("API_KEY", "secret")]);

        let policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::All,
            ..Default::default()
        };

        let result = populate_env(vars, &policy);
        assert_eq!(result.get("PATH"), Some(&"/usr/bin".to_string()));
        assert!(!result.contains_key("API_KEY"));
    }

    #[test]
    fn test_inherit_none() {
        let vars = make_vars(&[("PATH", "/usr/bin"), ("HOME", "/home")]);

        let mut policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::None,
            ignore_default_excludes: true,
            ..Default::default()
        };
        policy.set.insert("ONLY_VAR".to_string(), "yes".to_string());

        let result = populate_env(vars, &policy);
        assert!(!result.contains_key("PATH"));
        assert!(!result.contains_key("HOME"));
        assert_eq!(result.get("ONLY_VAR"), Some(&"yes".to_string()));
    }

    #[test]
    fn test_custom_exclude_patterns() {
        let vars = make_vars(&[
            ("PATH", "/usr/bin"),
            ("MY_CUSTOM_VAR", "value"),
            ("DEBUG_SETTING", "true"),
        ]);

        let policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::All,
            ignore_default_excludes: true,
            exclude: vec![EnvironmentVariablePattern::new("*CUSTOM*")],
            ..Default::default()
        };

        let result = populate_env(vars, &policy);
        assert_eq!(result.get("PATH"), Some(&"/usr/bin".to_string()));
        assert!(!result.contains_key("MY_CUSTOM_VAR"));
        assert_eq!(result.get("DEBUG_SETTING"), Some(&"true".to_string()));
    }

    #[test]
    fn test_set_overrides_existing() {
        let vars = make_vars(&[("PATH", "/usr/bin")]);

        let mut policy = ShellEnvironmentPolicy {
            ignore_default_excludes: true,
            ..Default::default()
        };
        policy
            .set
            .insert("PATH".to_string(), "/custom/path".to_string());

        let result = populate_env(vars, &policy);
        assert_eq!(result.get("PATH"), Some(&"/custom/path".to_string()));
    }

    #[test]
    fn test_multiple_excludes() {
        let vars = make_vars(&[
            ("PATH", "/usr/bin"),
            ("API_KEY", "k1"),
            ("SECRET", "s1"),
            ("AUTH_TOKEN", "t1"),
            ("NORMAL_VAR", "v1"),
        ]);

        let policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::All,
            ..Default::default()
        };

        let result = populate_env(vars, &policy);
        assert_eq!(result.get("PATH"), Some(&"/usr/bin".to_string()));
        assert_eq!(result.get("NORMAL_VAR"), Some(&"v1".to_string()));
        assert!(!result.contains_key("API_KEY"));
        assert!(!result.contains_key("SECRET"));
        assert!(!result.contains_key("AUTH_TOKEN"));
    }

    #[test]
    fn test_core_vars_list() {
        let vars = make_vars(&[
            ("HOME", "/home/user"),
            ("LOGNAME", "user"),
            ("PATH", "/usr/bin"),
            ("SHELL", "/bin/bash"),
            ("USER", "user"),
            ("USERNAME", "user"),
            ("TMPDIR", "/tmp"),
            ("TEMP", "/tmp"),
            ("TMP", "/tmp"),
            ("RANDOM_VAR", "value"),
        ]);

        let policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::Core,
            ignore_default_excludes: true,
            ..Default::default()
        };

        let result = populate_env(vars, &policy);
        assert!(result.contains_key("HOME"));
        assert!(result.contains_key("LOGNAME"));
        assert!(result.contains_key("PATH"));
        assert!(result.contains_key("SHELL"));
        assert!(result.contains_key("USER"));
        assert!(result.contains_key("USERNAME"));
        assert!(result.contains_key("TMPDIR"));
        assert!(result.contains_key("TEMP"));
        assert!(result.contains_key("TMP"));
        assert!(!result.contains_key("RANDOM_VAR"));
    }

    #[test]
    fn test_empty_policy() {
        let vars = make_vars(&[("PATH", "/usr/bin")]);
        let policy = ShellEnvironmentPolicy::default();
        let result = populate_env(vars, &policy);
        // Core inherits PATH, no KEY/SECRET/TOKEN to filter
        assert_eq!(result.get("PATH"), Some(&"/usr/bin".to_string()));
    }

    #[test]
    fn test_serialize_deserialize_policy() {
        let mut policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::All,
            exclude: vec![EnvironmentVariablePattern::new_case_insensitive(
                "*PRIVATE*",
            )],
            ignore_default_excludes: false,
            include_only: vec![],
            set: HashMap::new(),
        };
        policy.set.insert("CUSTOM".to_string(), "value".to_string());

        let json = serde_json::to_string(&policy).unwrap();
        let deserialized: ShellEnvironmentPolicy = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.inherit, ShellEnvironmentPolicyInherit::All);
        assert_eq!(deserialized.exclude.len(), 1);
        assert_eq!(deserialized.set.get("CUSTOM"), Some(&"value".to_string()));
    }

    // Additional tests for expanded coverage (N=297)

    #[test]
    fn test_pattern_multiple_wildcards() {
        let pattern = EnvironmentVariablePattern::new("A*B*C");
        assert!(pattern.matches("ABC"));
        assert!(pattern.matches("AxBxC"));
        assert!(pattern.matches("A_anything_B_anything_C"));
        assert!(!pattern.matches("ABCD")); // doesn't end with C
        assert!(!pattern.matches("AB")); // missing C
    }

    #[test]
    fn test_pattern_empty_string() {
        let pattern = EnvironmentVariablePattern::new("");
        assert!(pattern.matches(""));
        assert!(!pattern.matches("something"));
    }

    #[test]
    fn test_pattern_middle_wildcard() {
        let pattern = EnvironmentVariablePattern::new("PREFIX*SUFFIX");
        assert!(pattern.matches("PREFIXSUFFIX"));
        assert!(pattern.matches("PREFIX_MIDDLE_SUFFIX"));
        assert!(!pattern.matches("PREFIXONLY"));
        assert!(!pattern.matches("ONLYSUFFIX"));
    }

    #[test]
    fn test_pattern_case_sensitivity() {
        let case_sensitive = EnvironmentVariablePattern::new("PATH");
        assert!(case_sensitive.matches("PATH"));
        assert!(!case_sensitive.matches("path"));
        assert!(!case_sensitive.matches("Path"));

        let case_insensitive = EnvironmentVariablePattern::new_case_insensitive("PATH");
        assert!(case_insensitive.matches("PATH"));
        assert!(case_insensitive.matches("path"));
        assert!(case_insensitive.matches("Path"));
    }

    #[test]
    fn test_inherit_policy_enum_equality() {
        assert_eq!(
            ShellEnvironmentPolicyInherit::All,
            ShellEnvironmentPolicyInherit::All
        );
        assert_eq!(
            ShellEnvironmentPolicyInherit::None,
            ShellEnvironmentPolicyInherit::None
        );
        assert_eq!(
            ShellEnvironmentPolicyInherit::Core,
            ShellEnvironmentPolicyInherit::Core
        );
        assert_ne!(
            ShellEnvironmentPolicyInherit::All,
            ShellEnvironmentPolicyInherit::None
        );
    }

    #[test]
    fn test_inherit_policy_default() {
        let default = ShellEnvironmentPolicyInherit::default();
        assert_eq!(default, ShellEnvironmentPolicyInherit::Core);
    }

    #[test]
    fn test_exclude_and_include_only_combined() {
        let vars = make_vars(&[
            ("PATH", "/usr/bin"),
            ("HOME", "/home/user"),
            ("API_KEY", "secret"),
            ("MY_PATH_VAR", "/custom"),
        ]);

        let policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::All,
            ignore_default_excludes: true,
            exclude: vec![EnvironmentVariablePattern::new("API_KEY")],
            include_only: vec![EnvironmentVariablePattern::new("*PATH*")],
            ..Default::default()
        };

        let result = populate_env(vars, &policy);
        // Only PATH and MY_PATH_VAR should survive (match *PATH* and not API_KEY)
        assert!(result.contains_key("PATH"));
        assert!(result.contains_key("MY_PATH_VAR"));
        assert!(!result.contains_key("HOME"));
        assert!(!result.contains_key("API_KEY"));
    }

    #[test]
    fn test_set_adds_new_vars_even_with_none_inherit() {
        let vars = make_vars(&[("PATH", "/usr/bin")]);

        let mut policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::None,
            ..Default::default()
        };
        policy.set.insert("VAR1".to_string(), "val1".to_string());
        policy.set.insert("VAR2".to_string(), "val2".to_string());

        let result = populate_env(vars, &policy);
        assert!(!result.contains_key("PATH")); // Not inherited
        assert_eq!(result.get("VAR1"), Some(&"val1".to_string()));
        assert_eq!(result.get("VAR2"), Some(&"val2".to_string()));
    }

    #[test]
    fn test_default_excludes_filter_key_secret_token() {
        let vars = make_vars(&[
            ("NORMAL", "value"),
            ("MY_KEY", "k"),
            ("MY_SECRET", "s"),
            ("MY_TOKEN", "t"),
            ("KEYRING", "k"),
            ("SECRETSTORE", "s"),
            ("TOKENFILE", "t"),
        ]);

        let policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::All,
            ..Default::default()
        };

        let result = populate_env(vars, &policy);
        assert!(result.contains_key("NORMAL"));
        // All *KEY*, *SECRET*, *TOKEN* should be filtered
        assert!(!result.contains_key("MY_KEY"));
        assert!(!result.contains_key("MY_SECRET"));
        assert!(!result.contains_key("MY_TOKEN"));
        assert!(!result.contains_key("KEYRING"));
        assert!(!result.contains_key("SECRETSTORE"));
        assert!(!result.contains_key("TOKENFILE"));
    }

    #[test]
    fn test_empty_vars_input() {
        let vars: Vec<(String, String)> = vec![];
        let policy = ShellEnvironmentPolicy::default();
        let result = populate_env(vars, &policy);
        assert!(result.is_empty());
    }

    #[test]
    fn test_set_can_add_variables_with_keys_in_name() {
        // Even though *KEY* is excluded by default, explicit set should still work
        let vars = make_vars(&[]);

        let mut policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::None,
            ..Default::default()
        };
        policy
            .set
            .insert("MY_API_KEY".to_string(), "explicit".to_string());

        let result = populate_env(vars, &policy);
        // set happens after excludes, so it adds the var
        assert_eq!(result.get("MY_API_KEY"), Some(&"explicit".to_string()));
    }

    #[test]
    fn test_include_only_empty_keeps_all() {
        let vars = make_vars(&[("A", "1"), ("B", "2"), ("C", "3")]);

        let policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::All,
            ignore_default_excludes: true,
            include_only: vec![], // Empty means no filtering
            ..Default::default()
        };

        let result = populate_env(vars, &policy);
        assert!(result.contains_key("A"));
        assert!(result.contains_key("B"));
        assert!(result.contains_key("C"));
    }

    #[test]
    fn test_serialization_inherit_none() {
        let policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::None,
            ..Default::default()
        };
        let json = serde_json::to_string(&policy).unwrap();
        assert!(json.contains("\"inherit\":\"none\""));
    }

    #[test]
    fn test_serialization_inherit_all() {
        let policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::All,
            ..Default::default()
        };
        let json = serde_json::to_string(&policy).unwrap();
        assert!(json.contains("\"inherit\":\"all\""));
    }

    #[test]
    fn test_pattern_equality() {
        let p1 = EnvironmentVariablePattern::new("TEST");
        let p2 = EnvironmentVariablePattern::new("TEST");
        let p3 = EnvironmentVariablePattern::new("OTHER");

        assert_eq!(p1, p2);
        assert_ne!(p1, p3);
    }

    #[test]
    fn test_policy_clone() {
        let mut policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::All,
            exclude: vec![EnvironmentVariablePattern::new("TEST")],
            ..Default::default()
        };
        policy.set.insert("KEY".to_string(), "VALUE".to_string());

        let cloned = policy.clone();
        assert_eq!(cloned.inherit, ShellEnvironmentPolicyInherit::All);
        assert_eq!(cloned.exclude.len(), 1);
        assert_eq!(cloned.set.get("KEY"), Some(&"VALUE".to_string()));
    }

    #[test]
    fn test_populate_env_order_of_operations() {
        // Test that the order: inherit -> default excludes -> custom excludes -> set -> include_only
        let vars = make_vars(&[
            ("PATH", "/usr/bin"),
            ("MY_KEY", "k"),         // Will be filtered by default exclude
            ("CUSTOM_VAR", "c"),     // Will be filtered by custom exclude
            ("KEEP_THIS_PATH", "p"), // Will survive include_only
        ]);

        let mut policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::All,
            exclude: vec![EnvironmentVariablePattern::new("CUSTOM_VAR")],
            include_only: vec![EnvironmentVariablePattern::new("*PATH*")],
            ..Default::default()
        };
        policy.set.insert("SET_PATH".to_string(), "s".to_string());

        let result = populate_env(vars, &policy);
        assert!(result.contains_key("PATH"));
        assert!(result.contains_key("KEEP_THIS_PATH"));
        assert!(result.contains_key("SET_PATH")); // set happens before include_only
        assert!(!result.contains_key("MY_KEY")); // default exclude
        assert!(!result.contains_key("CUSTOM_VAR")); // custom exclude
    }

    #[test]
    fn test_pattern_debug_format() {
        let pattern = EnvironmentVariablePattern::new("*TEST*");
        let debug_str = format!("{:?}", pattern);
        assert!(debug_str.contains("TEST"));
    }

    #[test]
    fn test_policy_debug_format() {
        let policy = ShellEnvironmentPolicy::default();
        let debug_str = format!("{:?}", policy);
        assert!(debug_str.contains("ShellEnvironmentPolicy"));
    }
}
