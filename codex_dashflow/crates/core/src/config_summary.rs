//! Configuration summary module.
//!
//! Provides utilities to create human-readable summaries of configuration values.
//! These summaries are useful for CLI output, logging, and debugging.

use crate::config::Config;
use crate::sandbox_summary::summarize_sandbox_policy_short;
use crate::SandboxPolicy;

/// Creates a list of key-value pairs summarizing the effective configuration.
///
/// # Arguments
///
/// * `config` - The configuration to summarize
/// * `cwd` - The current working directory
/// * `sandbox_policy` - The effective sandbox policy
///
/// # Returns
///
/// A vector of (key, value) pairs suitable for display.
///
/// # Example
///
/// ```no_run
/// use codex_dashflow_core::config::Config;
/// use codex_dashflow_core::SandboxPolicy;
/// use codex_dashflow_core::config_summary::create_config_summary_entries;
/// use std::path::PathBuf;
///
/// let config = Config::default();
/// let entries = create_config_summary_entries(&config, &PathBuf::from("/home/user/project"), &SandboxPolicy::Native);
/// assert!(!entries.is_empty());
/// ```
pub fn create_config_summary_entries(
    config: &Config,
    cwd: &std::path::Path,
    sandbox_policy: &SandboxPolicy,
) -> Vec<(&'static str, String)> {
    let mut entries = vec![
        ("workdir", cwd.display().to_string()),
        ("model", config.model.clone()),
        ("sandbox", summarize_sandbox_policy_short(sandbox_policy)),
    ];

    if config.max_turns > 0 {
        entries.push(("max_turns", config.max_turns.to_string()));
    }

    if !config.mcp_servers.is_empty() {
        entries.push(("mcp_servers", config.mcp_servers.len().to_string()));
    }

    if config.collect_training {
        entries.push(("training", "enabled".to_string()));
    }

    entries
}

/// Creates a multi-line string summary of the configuration.
///
/// # Arguments
///
/// * `config` - The configuration to summarize
/// * `cwd` - The current working directory
/// * `sandbox_policy` - The effective sandbox policy
///
/// # Returns
///
/// A formatted string with one entry per line.
///
/// # Example
///
/// ```no_run
/// use codex_dashflow_core::config::Config;
/// use codex_dashflow_core::SandboxPolicy;
/// use codex_dashflow_core::config_summary::format_config_summary;
/// use std::path::PathBuf;
///
/// let config = Config::default();
/// let summary = format_config_summary(&config, &PathBuf::from("/home/user"), &SandboxPolicy::Native);
/// assert!(summary.contains("model"));
/// ```
pub fn format_config_summary(
    config: &Config,
    cwd: &std::path::Path,
    sandbox_policy: &SandboxPolicy,
) -> String {
    let entries = create_config_summary_entries(config, cwd, sandbox_policy);
    entries
        .iter()
        .map(|(k, v)| format!("{}: {}", k, v))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Creates a compact single-line summary of the configuration.
///
/// # Arguments
///
/// * `config` - The configuration to summarize
/// * `sandbox_policy` - The effective sandbox policy
///
/// # Returns
///
/// A compact string like "model=gpt-4, sandbox=native".
///
/// # Example
///
/// ```no_run
/// use codex_dashflow_core::config::Config;
/// use codex_dashflow_core::SandboxPolicy;
/// use codex_dashflow_core::config_summary::format_config_compact;
///
/// let config = Config::default();
/// let summary = format_config_compact(&config, &SandboxPolicy::Native);
/// assert!(summary.contains("model="));
/// ```
pub fn format_config_compact(config: &Config, sandbox_policy: &SandboxPolicy) -> String {
    let mut parts = vec![
        format!("model={}", config.model),
        format!("sandbox={}", summarize_sandbox_policy_short(sandbox_policy)),
    ];

    if config.max_turns > 0 {
        parts.push(format!("max_turns={}", config.max_turns));
    }

    parts.join(", ")
}

/// Computes the maximum key width from summary entries.
///
/// Useful for aligned formatting in CLI output.
pub fn max_key_width(entries: &[(&str, String)]) -> usize {
    entries.iter().map(|(k, _)| k.len()).max().unwrap_or(0)
}

/// Formats entries with aligned columns.
///
/// # Arguments
///
/// * `entries` - Key-value pairs to format
/// * `separator` - Separator between key and value (e.g., ": " or " = ")
///
/// # Returns
///
/// A multi-line string with keys right-padded for alignment.
pub fn format_aligned(entries: &[(&str, String)], separator: &str) -> String {
    let width = max_key_width(entries);
    entries
        .iter()
        .map(|(k, v)| format!("{:width$}{}{}", k, separator, v, width = width))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_create_config_summary_entries() {
        let config = Config::default();
        let entries =
            create_config_summary_entries(&config, &PathBuf::from("/test"), &SandboxPolicy::Native);

        assert!(!entries.is_empty());
        // Should have workdir, model, sandbox at minimum
        assert!(entries.iter().any(|(k, _)| *k == "workdir"));
        assert!(entries.iter().any(|(k, _)| *k == "model"));
        assert!(entries.iter().any(|(k, _)| *k == "sandbox"));
    }

    #[test]
    fn test_create_config_summary_with_max_turns() {
        let config = Config {
            max_turns: 10,
            ..Default::default()
        };
        let entries =
            create_config_summary_entries(&config, &PathBuf::from("/test"), &SandboxPolicy::Native);

        let max_turns = entries.iter().find(|(k, _)| *k == "max_turns");
        assert!(max_turns.is_some());
        assert_eq!(max_turns.unwrap().1, "10");
    }

    #[test]
    fn test_create_config_summary_with_training() {
        let config = Config {
            collect_training: true,
            ..Default::default()
        };
        let entries =
            create_config_summary_entries(&config, &PathBuf::from("/test"), &SandboxPolicy::Native);

        let training = entries.iter().find(|(k, _)| *k == "training");
        assert!(training.is_some());
        assert_eq!(training.unwrap().1, "enabled");
    }

    #[test]
    fn test_format_config_summary() {
        let config = Config::default();
        let summary = format_config_summary(
            &config,
            &PathBuf::from("/home/user"),
            &SandboxPolicy::Native,
        );

        assert!(summary.contains("workdir"));
        assert!(summary.contains("model"));
        assert!(summary.contains("/home/user"));
    }

    #[test]
    fn test_format_config_compact() {
        let config = Config::default();
        let summary = format_config_compact(&config, &SandboxPolicy::Native);

        assert!(summary.contains("model="));
        assert!(summary.contains("sandbox=native"));
        assert!(summary.contains(", "));
    }

    #[test]
    fn test_format_config_compact_with_max_turns() {
        let config = Config {
            max_turns: 5,
            ..Default::default()
        };
        let summary = format_config_compact(&config, &SandboxPolicy::Native);

        assert!(summary.contains("max_turns=5"));
    }

    #[test]
    fn test_max_key_width() {
        let entries = vec![
            ("a", "1".to_string()),
            ("longer", "2".to_string()),
            ("x", "3".to_string()),
        ];
        assert_eq!(max_key_width(&entries), 6); // "longer" is 6 chars
    }

    #[test]
    fn test_max_key_width_empty() {
        let entries: Vec<(&str, String)> = vec![];
        assert_eq!(max_key_width(&entries), 0);
    }

    #[test]
    fn test_format_aligned() {
        let entries = vec![
            ("a", "1".to_string()),
            ("bb", "2".to_string()),
            ("ccc", "3".to_string()),
        ];
        let formatted = format_aligned(&entries, ": ");
        let lines: Vec<_> = formatted.lines().collect();

        assert_eq!(lines.len(), 3);
        // All separators should align
        assert!(lines[0].contains("a  : 1"));
        assert!(lines[1].contains("bb : 2"));
        assert!(lines[2].contains("ccc: 3"));
    }

    #[test]
    fn test_sandbox_in_summary() {
        let config = Config::default();

        // Test with Docker sandbox
        let entries = create_config_summary_entries(
            &config,
            &PathBuf::from("/test"),
            &SandboxPolicy::Docker {
                image: Some("myimage".to_string()),
            },
        );
        let sandbox = entries.iter().find(|(k, _)| *k == "sandbox").unwrap();
        assert!(sandbox.1.contains("docker"));
    }

    #[test]
    fn test_create_config_summary_with_mcp_servers() {
        use codex_dashflow_mcp::{McpServerConfig, McpTransport};

        let config = Config {
            mcp_servers: vec![
                McpServerConfig {
                    name: "server1".to_string(),
                    transport: McpTransport::Stdio {
                        command: "node".to_string(),
                        args: vec![],
                    },
                    env: Default::default(),
                    cwd: None,
                    timeout_secs: 30,
                },
                McpServerConfig {
                    name: "server2".to_string(),
                    transport: McpTransport::Stdio {
                        command: "python".to_string(),
                        args: vec![],
                    },
                    env: Default::default(),
                    cwd: None,
                    timeout_secs: 30,
                },
            ],
            ..Default::default()
        };
        let entries =
            create_config_summary_entries(&config, &PathBuf::from("/test"), &SandboxPolicy::Native);

        let mcp_servers = entries.iter().find(|(k, _)| *k == "mcp_servers");
        assert!(mcp_servers.is_some());
        assert_eq!(mcp_servers.unwrap().1, "2");
    }

    #[test]
    fn test_format_config_summary_multiline() {
        let config = Config {
            max_turns: 5,
            ..Default::default()
        };
        let summary =
            format_config_summary(&config, &PathBuf::from("/test"), &SandboxPolicy::Native);

        let lines: Vec<_> = summary.lines().collect();
        assert!(lines.len() >= 3); // workdir, model, sandbox, possibly max_turns
        for line in &lines {
            assert!(line.contains(": "));
        }
    }

    #[test]
    fn test_format_aligned_different_separators() {
        let entries = vec![("k", "v".to_string())];

        let sep1 = format_aligned(&entries, ": ");
        let sep2 = format_aligned(&entries, " = ");
        let sep3 = format_aligned(&entries, " -> ");

        assert!(sep1.contains(": "));
        assert!(sep2.contains(" = "));
        assert!(sep3.contains(" -> "));
    }

    #[test]
    fn test_max_key_width_single_entry() {
        let entries = vec![("test", "value".to_string())];
        assert_eq!(max_key_width(&entries), 4);
    }

    #[test]
    fn test_format_aligned_single_entry() {
        let entries = vec![("key", "value".to_string())];
        let formatted = format_aligned(&entries, ": ");
        assert_eq!(formatted, "key: value");
    }

    #[test]
    fn test_sandbox_policy_none() {
        let config = Config::default();
        let entries =
            create_config_summary_entries(&config, &PathBuf::from("/test"), &SandboxPolicy::None);
        let sandbox = entries.iter().find(|(k, _)| *k == "sandbox").unwrap();
        // SandboxPolicy::None should produce "none" in the summary
        assert!(sandbox.1.contains("none"));
    }

    #[test]
    fn test_format_config_compact_no_max_turns() {
        let config = Config {
            max_turns: 0,
            ..Default::default()
        };
        let summary = format_config_compact(&config, &SandboxPolicy::Native);

        assert!(!summary.contains("max_turns"));
    }

    #[test]
    fn test_cwd_with_spaces() {
        let config = Config::default();
        let entries = create_config_summary_entries(
            &config,
            &PathBuf::from("/path with spaces/project"),
            &SandboxPolicy::Native,
        );

        let workdir = entries.iter().find(|(k, _)| *k == "workdir").unwrap();
        assert_eq!(workdir.1, "/path with spaces/project");
    }

    #[test]
    fn test_cwd_with_unicode() {
        let config = Config::default();
        let entries = create_config_summary_entries(
            &config,
            &PathBuf::from("/путь/项目/プロジェクト"),
            &SandboxPolicy::Native,
        );

        let workdir = entries.iter().find(|(k, _)| *k == "workdir").unwrap();
        assert!(workdir.1.contains("путь"));
        assert!(workdir.1.contains("项目"));
    }

    #[test]
    fn test_format_aligned_preserves_order() {
        let entries = vec![
            ("first", "1".to_string()),
            ("second", "2".to_string()),
            ("third", "3".to_string()),
        ];
        let formatted = format_aligned(&entries, ": ");
        let lines: Vec<_> = formatted.lines().collect();

        assert!(lines[0].contains("first"));
        assert!(lines[1].contains("second"));
        assert!(lines[2].contains("third"));
    }

    #[test]
    fn test_format_config_compact_parts_separated_by_comma() {
        let config = Config::default();
        let summary = format_config_compact(&config, &SandboxPolicy::Native);

        let parts: Vec<_> = summary.split(", ").collect();
        assert!(parts.len() >= 2);
        // First part should be model=
        assert!(parts[0].starts_with("model="));
        // Second part should be sandbox=
        assert!(parts[1].starts_with("sandbox="));
    }

    #[test]
    fn test_entries_order_is_consistent() {
        let config = Config::default();
        let entries1 =
            create_config_summary_entries(&config, &PathBuf::from("/test"), &SandboxPolicy::Native);
        let entries2 =
            create_config_summary_entries(&config, &PathBuf::from("/test"), &SandboxPolicy::Native);

        // Order should be deterministic
        assert_eq!(entries1.len(), entries2.len());
        for (e1, e2) in entries1.iter().zip(entries2.iter()) {
            assert_eq!(e1.0, e2.0);
            assert_eq!(e1.1, e2.1);
        }
    }

    #[test]
    fn test_default_config_has_required_entries() {
        let config = Config::default();
        let entries =
            create_config_summary_entries(&config, &PathBuf::from("/"), &SandboxPolicy::Native);

        // These should always be present
        let keys: Vec<_> = entries.iter().map(|(k, _)| *k).collect();
        assert!(keys.contains(&"workdir"));
        assert!(keys.contains(&"model"));
        assert!(keys.contains(&"sandbox"));
    }

    #[test]
    fn test_format_aligned_empty_values() {
        let entries = vec![("key1", "".to_string()), ("key2", "value".to_string())];
        let formatted = format_aligned(&entries, ": ");
        let lines: Vec<_> = formatted.lines().collect();

        assert!(lines[0].ends_with(": "));
        assert!(lines[1].contains("value"));
    }
}
