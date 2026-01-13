//! Sandbox summary module.
//!
//! Provides human-readable summaries of sandbox policies for CLI output and logging.

use crate::SandboxPolicy;

/// Generates a human-readable summary of a sandbox policy.
///
/// # Arguments
///
/// * `policy` - The sandbox policy to summarize
///
/// # Returns
///
/// A string describing the sandbox policy in human-readable form.
///
/// # Example
///
/// ```no_run
/// use codex_dashflow_core::{SandboxPolicy, sandbox_summary::summarize_sandbox_policy};
///
/// assert_eq!(summarize_sandbox_policy(&SandboxPolicy::None), "none");
/// assert_eq!(summarize_sandbox_policy(&SandboxPolicy::Native), "native (Seatbelt/Landlock)");
/// ```
pub fn summarize_sandbox_policy(policy: &SandboxPolicy) -> String {
    match policy {
        SandboxPolicy::None => "none".to_string(),
        SandboxPolicy::Native => "native (Seatbelt/Landlock)".to_string(),
        SandboxPolicy::Docker { image } => {
            if let Some(img) = image {
                format!("docker [image: {}]", img)
            } else {
                "docker [default image]".to_string()
            }
        }
    }
}

/// Generates a short summary of a sandbox policy (for compact displays).
///
/// # Arguments
///
/// * `policy` - The sandbox policy to summarize
///
/// # Returns
///
/// A short string suitable for compact UI elements.
///
/// # Example
///
/// ```no_run
/// use codex_dashflow_core::{SandboxPolicy, sandbox_summary::summarize_sandbox_policy_short};
///
/// assert_eq!(summarize_sandbox_policy_short(&SandboxPolicy::Native), "native");
/// ```
pub fn summarize_sandbox_policy_short(policy: &SandboxPolicy) -> String {
    match policy {
        SandboxPolicy::None => "none".to_string(),
        SandboxPolicy::Native => "native".to_string(),
        SandboxPolicy::Docker { image } => {
            if let Some(img) = image {
                format!("docker:{}", img)
            } else {
                "docker".to_string()
            }
        }
    }
}

/// Returns the security level indicator for a sandbox policy.
///
/// # Arguments
///
/// * `policy` - The sandbox policy to evaluate
///
/// # Returns
///
/// A tuple of (level_name, emoji) for UI display.
///
/// # Example
///
/// ```no_run
/// use codex_dashflow_core::{SandboxPolicy, sandbox_summary::sandbox_security_level};
///
/// let (level, emoji) = sandbox_security_level(&SandboxPolicy::None);
/// assert_eq!(level, "danger");
/// ```
pub fn sandbox_security_level(policy: &SandboxPolicy) -> (&'static str, &'static str) {
    match policy {
        SandboxPolicy::None => ("danger", "!!!"),
        SandboxPolicy::Native => ("secure", "[+]"),
        SandboxPolicy::Docker { .. } => ("isolated", "[=]"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summarize_none() {
        assert_eq!(summarize_sandbox_policy(&SandboxPolicy::None), "none");
    }

    #[test]
    fn test_summarize_native() {
        let summary = summarize_sandbox_policy(&SandboxPolicy::Native);
        assert!(summary.contains("native"));
        assert!(summary.contains("Seatbelt") || summary.contains("Landlock"));
    }

    #[test]
    fn test_summarize_docker_default() {
        let summary = summarize_sandbox_policy(&SandboxPolicy::Docker { image: None });
        assert!(summary.contains("docker"));
        assert!(summary.contains("default"));
    }

    #[test]
    fn test_summarize_docker_custom_image() {
        let summary = summarize_sandbox_policy(&SandboxPolicy::Docker {
            image: Some("myimage:latest".to_string()),
        });
        assert!(summary.contains("docker"));
        assert!(summary.contains("myimage:latest"));
    }

    #[test]
    fn test_summarize_short_none() {
        assert_eq!(summarize_sandbox_policy_short(&SandboxPolicy::None), "none");
    }

    #[test]
    fn test_summarize_short_native() {
        assert_eq!(
            summarize_sandbox_policy_short(&SandboxPolicy::Native),
            "native"
        );
    }

    #[test]
    fn test_summarize_short_docker_default() {
        assert_eq!(
            summarize_sandbox_policy_short(&SandboxPolicy::Docker { image: None }),
            "docker"
        );
    }

    #[test]
    fn test_summarize_short_docker_custom() {
        assert_eq!(
            summarize_sandbox_policy_short(&SandboxPolicy::Docker {
                image: Some("alpine".to_string())
            }),
            "docker:alpine"
        );
    }

    #[test]
    fn test_security_level_none() {
        let (level, indicator) = sandbox_security_level(&SandboxPolicy::None);
        assert_eq!(level, "danger");
        assert!(indicator.contains("!"));
    }

    #[test]
    fn test_security_level_native() {
        let (level, indicator) = sandbox_security_level(&SandboxPolicy::Native);
        assert_eq!(level, "secure");
        assert!(indicator.contains("+"));
    }

    #[test]
    fn test_security_level_docker() {
        let (level, _indicator) = sandbox_security_level(&SandboxPolicy::Docker { image: None });
        assert_eq!(level, "isolated");
    }
}
