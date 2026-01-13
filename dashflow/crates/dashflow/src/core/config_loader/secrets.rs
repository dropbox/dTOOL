// Config loader clippy exceptions:
// - clone_on_ref_ptr: Arc::clone() for sharing config state
// - needless_pass_by_value: API ergonomics - String parameters are cheap to clone
// - redundant_clone: Clone for ownership clarity when passing secrets
#![allow(clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! Secret handling and environment variable resolution

use crate::core::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::env;

/// Reference to a secret value, either from environment or inline
///
/// # Security Note
///
/// Inline secrets should NEVER be committed to version control.
/// They are provided only for testing and local development.
/// Production configurations must use environment variable references.
///
/// # Example
///
/// ```yaml
/// # Recommended: Environment variable
/// api_key:
///   env: OPENAI_API_KEY
///
/// # NOT recommended (testing only):
/// api_key: sk-...
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SecretReference {
    /// Reference to an environment variable
    EnvVar {
        /// Name of the environment variable
        env: String,
    },

    /// Inline secret value (TESTING ONLY - never commit to git)
    Inline(String),
}

impl SecretReference {
    /// Resolve the secret to its actual value
    ///
    /// For environment variables, reads from `std::env::var()`.
    /// For inline values, returns the value directly.
    ///
    /// # Errors
    ///
    /// Returns an error if the environment variable is not set.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::core::config_loader::SecretReference;
    ///
    /// let secret = SecretReference::EnvVar {
    ///     env: "OPENAI_API_KEY".to_string(),
    /// };
    ///
    /// let api_key = secret.resolve()?;
    /// ```
    pub fn resolve(&self) -> Result<String> {
        match self {
            SecretReference::EnvVar { env } => env::var(env).map_err(|e| {
                Error::Configuration(format!(
                    "Environment variable '{env}' not set ({e}). \
                         Please set it before loading this configuration."
                ))
            }),
            SecretReference::Inline(value) => Ok(value.clone()),
        }
    }

    /// Create a secret reference from an environment variable name
    pub fn from_env(env_var: impl Into<String>) -> Self {
        SecretReference::EnvVar {
            env: env_var.into(),
        }
    }

    /// Create an inline secret (TESTING ONLY)
    ///
    /// # Security Warning
    ///
    /// This should NEVER be used in production or committed to version control.
    pub fn from_inline(value: impl Into<String>) -> Self {
        SecretReference::Inline(value.into())
    }
}

/// Expand environment variables in a string
///
/// Supports the following syntaxes:
/// - `${VAR}` - Environment variable (recommended)
/// - `$VAR` - Environment variable (shell-style)
///
/// Variables that are not set are left unchanged.
///
/// # Example
///
/// ```rust
/// use dashflow::core::config_loader::expand_env_vars;
///
/// std::env::set_var("HOME", "/home/user");
/// std::env::set_var("USER", "alice");
///
/// assert_eq!(
///     expand_env_vars("${HOME}/files"),
///     "/home/user/files"
/// );
///
/// assert_eq!(
///     expand_env_vars("User: $USER, Home: ${HOME}"),
///     "User: alice, Home: /home/user"
/// );
///
/// // Unset variables are left unchanged
/// assert_eq!(
///     expand_env_vars("${UNSET_VAR}"),
///     "${UNSET_VAR}"
/// );
/// ```
#[must_use]
#[allow(clippy::expect_used)] // Safe: regex patterns are compile-time constants
pub fn expand_env_vars(input: &str) -> String {
    // Match ${VAR} syntax
    let braced_re = regex::Regex::new(r"\$\{([A-Z_][A-Z0-9_]*)\}")
        .expect("static braced env var regex pattern");
    let result = braced_re.replace_all(input, |caps: &regex::Captures| {
        env::var(&caps[1]).unwrap_or_else(|_| caps[0].to_string())
    });

    // Match $VAR syntax
    let unbraced_re =
        regex::Regex::new(r"\$([A-Z_][A-Z0-9_]*)").expect("static unbraced env var regex pattern");
    unbraced_re
        .replace_all(&result, |caps: &regex::Captures| {
            env::var(&caps[1]).unwrap_or_else(|_| caps[0].to_string())
        })
        .to_string()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use crate::test_prelude::*;
    use std::sync::Mutex;

    // Mutex to serialize env-var-dependent tests (parallel execution causes races)
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_secret_reference_env_var() {
        let _guard = ENV_MUTEX.lock().unwrap();
        env::set_var("TEST_SECRET", "secret_value");

        let secret = SecretReference::EnvVar {
            env: "TEST_SECRET".to_string(),
        };

        let result = secret.resolve().unwrap();
        env::remove_var("TEST_SECRET");
        assert_eq!(result, "secret_value");
    }

    #[test]
    fn test_secret_reference_env_var_missing() {
        let _guard = ENV_MUTEX.lock().unwrap();
        env::remove_var("MISSING_VAR");

        let secret = SecretReference::EnvVar {
            env: "MISSING_VAR".to_string(),
        };

        assert!(secret.resolve().is_err());
    }

    #[test]
    fn test_secret_reference_inline() {
        let secret = SecretReference::Inline("inline_secret".to_string());
        assert_eq!(secret.resolve().unwrap(), "inline_secret");
    }

    #[test]
    fn test_secret_reference_from_env() {
        let secret = SecretReference::from_env("MY_KEY");
        matches!(secret, SecretReference::EnvVar { env } if env == "MY_KEY");
    }

    #[test]
    fn test_secret_reference_from_inline() {
        let secret = SecretReference::from_inline("test");
        matches!(secret, SecretReference::Inline(val) if val == "test");
    }

    #[test]
    fn test_expand_env_vars_braced() {
        let _guard = ENV_MUTEX.lock().unwrap();
        env::set_var("TEST_VAR", "test_value");
        env::set_var("ANOTHER_VAR", "another");

        let r1 = expand_env_vars("${TEST_VAR}");
        let r2 = expand_env_vars("prefix_${TEST_VAR}_suffix");
        let r3 = expand_env_vars("${TEST_VAR} and ${ANOTHER_VAR}");

        env::remove_var("TEST_VAR");
        env::remove_var("ANOTHER_VAR");

        assert_eq!(r1, "test_value");
        assert_eq!(r2, "prefix_test_value_suffix");
        assert_eq!(r3, "test_value and another");
    }

    #[test]
    fn test_expand_env_vars_unbraced() {
        let _guard = ENV_MUTEX.lock().unwrap();
        // Save original values to restore after test
        let orig_home = env::var("HOME").ok();
        let orig_user = env::var("USER").ok();

        env::set_var("HOME", "/home/user");
        env::set_var("USER", "alice");

        let r1 = expand_env_vars("$HOME");
        let r2 = expand_env_vars("$HOME/files");
        let r3 = expand_env_vars("User: $USER");

        // Restore original values or remove if they didn't exist
        match orig_home {
            Some(v) => env::set_var("HOME", v),
            None => env::remove_var("HOME"),
        }
        match orig_user {
            Some(v) => env::set_var("USER", v),
            None => env::remove_var("USER"),
        }

        assert_eq!(r1, "/home/user");
        assert_eq!(r2, "/home/user/files");
        assert_eq!(r3, "User: alice");
    }

    #[test]
    fn test_expand_env_vars_missing() {
        let _guard = ENV_MUTEX.lock().unwrap();
        env::remove_var("UNSET_VAR");

        // Missing variables are left unchanged
        assert_eq!(expand_env_vars("${UNSET_VAR}"), "${UNSET_VAR}");
        assert_eq!(expand_env_vars("$UNSET_VAR"), "$UNSET_VAR");
    }

    #[test]
    fn test_expand_env_vars_mixed() {
        let _guard = ENV_MUTEX.lock().unwrap();
        env::set_var("VAR1", "value1");
        env::set_var("VAR2", "value2");

        let result = expand_env_vars("${VAR1} and $VAR2");

        env::remove_var("VAR1");
        env::remove_var("VAR2");

        assert_eq!(result, "value1 and value2");
    }

    #[test]
    fn test_serde_secret_reference() {
        // Test environment variable serialization
        let env_secret = SecretReference::EnvVar {
            env: "MY_KEY".to_string(),
        };
        let json = serde_json::to_string(&env_secret).unwrap();
        assert_eq!(json, r#"{"env":"MY_KEY"}"#);

        let deserialized: SecretReference = serde_json::from_str(&json).unwrap();
        matches!(deserialized, SecretReference::EnvVar { env } if env == "MY_KEY");

        // Test inline serialization
        let inline_secret = SecretReference::Inline("secret".to_string());
        let json = serde_json::to_string(&inline_secret).unwrap();
        assert_eq!(json, r#""secret""#);

        let deserialized: SecretReference = serde_json::from_str(&json).unwrap();
        matches!(deserialized, SecretReference::Inline(val) if val == "secret");
    }
}
