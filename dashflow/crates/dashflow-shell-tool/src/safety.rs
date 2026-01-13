//! Command safety analysis for shell tool
//!
//! Analyzes shell commands to detect potentially dangerous operations.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::OnceLock;

/// Severity level of a command
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Safe command - no special handling needed
    Safe,
    /// Unknown risk - might need review
    Unknown,
    /// Dangerous command - should prompt for confirmation
    Dangerous,
    /// Forbidden command - should never be executed
    Forbidden,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Safe => write!(f, "safe"),
            Severity::Unknown => write!(f, "unknown"),
            Severity::Dangerous => write!(f, "dangerous"),
            Severity::Forbidden => write!(f, "forbidden"),
        }
    }
}

/// Result of command analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// Command that was analyzed
    pub command: String,
    /// Determined severity level
    pub severity: Severity,
    /// Reasons for the severity determination
    pub reasons: Vec<String>,
    /// Commands extracted from the input
    pub commands: Vec<String>,
    /// Whether the command modifies the filesystem
    pub modifies_filesystem: bool,
    /// Whether the command accesses network
    pub accesses_network: bool,
    /// Whether the command modifies system state
    pub modifies_system: bool,
}

/// Safety configuration for command analysis
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SafetyConfig {
    /// Commands that are always safe (first token)
    #[serde(default)]
    pub safe_commands: HashSet<String>,

    /// Command prefixes that are always safe
    #[serde(default)]
    pub safe_prefixes: HashSet<String>,

    /// Patterns that are forbidden (regex)
    #[serde(default)]
    pub forbidden_patterns: Vec<String>,

    /// Patterns that are dangerous (regex)
    #[serde(default)]
    pub dangerous_patterns: Vec<String>,

    /// Whether to allow network access
    #[serde(default = "default_true")]
    pub allow_network: bool,

    /// Whether to allow filesystem modifications
    #[serde(default = "default_true")]
    pub allow_filesystem_write: bool,

    /// Whether to allow system modifications
    #[serde(default)]
    pub allow_system_modify: bool,
}

fn default_true() -> bool {
    true
}

impl SafetyConfig {
    /// Create a restrictive safety config (safe by default)
    #[must_use]
    pub fn restrictive() -> Self {
        let mut safe = HashSet::new();
        for cmd in &[
            "ls", "pwd", "echo", "cat", "head", "tail", "wc", "date", "whoami", "hostname",
            "uname", "env", "printenv", "which", "type", "file", "stat", "du", "df", "uptime",
            "ps", "top", "free",
        ] {
            safe.insert((*cmd).to_string());
        }

        Self {
            safe_commands: safe,
            safe_prefixes: HashSet::new(),
            forbidden_patterns: vec![
                r"rm\s+-rf\s+/".to_string(),
                r"rm\s+-fr\s+/".to_string(),
                r"mkfs".to_string(),
                r"dd\s+if=.*of=/dev".to_string(),
                r":\(\)\{.*\}".to_string(), // fork bomb
                r">\s*/dev/sd".to_string(),
                r"chmod\s+-R\s+777\s+/".to_string(),
                r"chown\s+-R.*\s+/".to_string(),
            ],
            dangerous_patterns: vec![
                r"rm\s+-r".to_string(),
                r"sudo".to_string(),
                r"su\s+".to_string(),
                r"chmod".to_string(),
                r"chown".to_string(),
                r"kill".to_string(),
                r"pkill".to_string(),
                r"shutdown".to_string(),
                r"reboot".to_string(),
                r"systemctl".to_string(),
                r"service\s+".to_string(),
            ],
            allow_network: false,
            allow_filesystem_write: false,
            allow_system_modify: false,
        }
    }

    /// Create a permissive safety config (for trusted environments)
    #[must_use]
    pub fn permissive() -> Self {
        Self {
            safe_commands: HashSet::new(),
            safe_prefixes: HashSet::new(),
            forbidden_patterns: vec![
                r"rm\s+-rf\s+/\s*$".to_string(),
                r"rm\s+-fr\s+/\s*$".to_string(),
                r"mkfs".to_string(),
                r":\(\)\{.*\}".to_string(), // fork bomb
            ],
            dangerous_patterns: Vec::new(),
            allow_network: true,
            allow_filesystem_write: true,
            allow_system_modify: false,
        }
    }

    /// Add safe commands
    pub fn with_safe_commands(mut self, commands: impl IntoIterator<Item = String>) -> Self {
        self.safe_commands.extend(commands);
        self
    }

    /// Add forbidden patterns
    pub fn with_forbidden_patterns(mut self, patterns: impl IntoIterator<Item = String>) -> Self {
        self.forbidden_patterns.extend(patterns);
        self
    }

    /// Add dangerous patterns
    pub fn with_dangerous_patterns(mut self, patterns: impl IntoIterator<Item = String>) -> Self {
        self.dangerous_patterns.extend(patterns);
        self
    }
}

/// Command safety analyzer
pub struct CommandAnalyzer {
    config: SafetyConfig,
    forbidden_regexes: Vec<Regex>,
    dangerous_regexes: Vec<Regex>,
}

impl CommandAnalyzer {
    /// Create a new analyzer with the given config
    #[must_use]
    pub fn new(config: SafetyConfig) -> Self {
        // Use bounded regex compilation for config-provided patterns to prevent ReDoS
        let forbidden_regexes = config
            .forbidden_patterns
            .iter()
            .filter_map(|p| {
                regex::RegexBuilder::new(p)
                    .size_limit(256 * 1024)
                    .dfa_size_limit(256 * 1024)
                    .build()
                    .ok()
            })
            .collect();

        let dangerous_regexes = config
            .dangerous_patterns
            .iter()
            .filter_map(|p| {
                regex::RegexBuilder::new(p)
                    .size_limit(256 * 1024)
                    .dfa_size_limit(256 * 1024)
                    .build()
                    .ok()
            })
            .collect();

        Self {
            config,
            forbidden_regexes,
            dangerous_regexes,
        }
    }

    /// Create analyzer with default restrictive config
    #[must_use]
    pub fn restrictive() -> Self {
        Self::new(SafetyConfig::restrictive())
    }

    /// Create analyzer with permissive config
    #[must_use]
    pub fn permissive() -> Self {
        Self::new(SafetyConfig::permissive())
    }

    /// Analyze a command and return safety assessment
    #[must_use]
    pub fn analyze(&self, command: &str) -> AnalysisResult {
        let command = command.trim();
        let mut reasons = Vec::new();
        let mut severity = Severity::Unknown;

        // Extract individual commands (split on pipes, semicolons, etc.)
        let commands = self.extract_commands(command);

        // Check filesystem modification
        let modifies_filesystem = self.modifies_filesystem(command);
        if modifies_filesystem && !self.config.allow_filesystem_write {
            reasons.push("Command modifies filesystem".to_string());
            severity = severity.max(Severity::Dangerous);
        }

        // Check network access
        let accesses_network = self.accesses_network(command);
        if accesses_network && !self.config.allow_network {
            reasons.push("Command accesses network".to_string());
            severity = severity.max(Severity::Dangerous);
        }

        // Check system modification
        let modifies_system = self.modifies_system(command);
        if modifies_system && !self.config.allow_system_modify {
            reasons.push("Command modifies system state".to_string());
            severity = severity.max(Severity::Dangerous);
        }

        // Check forbidden patterns
        for regex in &self.forbidden_regexes {
            if regex.is_match(command) {
                reasons.push(format!("Matches forbidden pattern: {}", regex.as_str()));
                severity = Severity::Forbidden;
            }
        }

        // Check dangerous patterns (only if not already forbidden)
        if severity != Severity::Forbidden {
            for regex in &self.dangerous_regexes {
                if regex.is_match(command) {
                    reasons.push(format!("Matches dangerous pattern: {}", regex.as_str()));
                    severity = severity.max(Severity::Dangerous);
                }
            }
        }

        // Check if explicitly safe
        if severity < Severity::Dangerous {
            let first_command = commands.first().map(|c| self.first_token(c)).unwrap_or("");
            if self.config.safe_commands.contains(first_command) {
                severity = Severity::Safe;
                reasons.clear();
                reasons.push(format!("Command '{}' is in safe list", first_command));
            }

            // Check safe prefixes
            for prefix in &self.config.safe_prefixes {
                if command.starts_with(prefix) {
                    severity = Severity::Safe;
                    reasons.clear();
                    reasons.push(format!("Command starts with safe prefix '{}'", prefix));
                    break;
                }
            }
        }

        AnalysisResult {
            command: command.to_string(),
            severity,
            reasons,
            commands,
            modifies_filesystem,
            accesses_network,
            modifies_system,
        }
    }

    /// Extract individual commands from a compound command
    fn extract_commands(&self, command: &str) -> Vec<String> {
        // Split on common shell operators
        let mut commands = Vec::new();

        // Simple split on common separators (not perfect but covers most cases)
        for part in command.split([';', '|', '&']) {
            let trimmed = part.trim();
            if !trimmed.is_empty() {
                commands.push(trimmed.to_string());
            }
        }

        // Handle $() and `` command substitution
        let subst_regex = get_subst_regex();
        for cap in subst_regex.captures_iter(command) {
            if let Some(inner) = cap.get(1).or(cap.get(2)) {
                commands.push(inner.as_str().to_string());
            }
        }

        commands
    }

    /// Get first token (command name) from a command string
    fn first_token<'a>(&self, command: &'a str) -> &'a str {
        command.split_whitespace().next().unwrap_or("")
    }

    /// Check if command modifies filesystem
    fn modifies_filesystem(&self, command: &str) -> bool {
        let patterns = get_fs_modify_patterns();
        patterns.iter().any(|p| p.is_match(command))
    }

    /// Check if command accesses network
    fn accesses_network(&self, command: &str) -> bool {
        let patterns = get_network_patterns();
        patterns.iter().any(|p| p.is_match(command))
    }

    /// Check if command modifies system state
    fn modifies_system(&self, command: &str) -> bool {
        let patterns = get_system_modify_patterns();
        patterns.iter().any(|p| p.is_match(command))
    }
}

// Lazy-initialized regex patterns
// SAFETY: All Regex::new() calls use hardcoded patterns verified at development time.
// Static regex patterns in OnceLock are a standard Rust idiom for lazy initialization.
#[allow(clippy::unwrap_used)]
fn get_subst_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\$\(([^)]+)\)|`([^`]+)`").unwrap())
}

#[allow(clippy::unwrap_used)]
fn get_fs_modify_patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            Regex::new(r"\brm\b").unwrap(),
            Regex::new(r"\bmv\b").unwrap(),
            Regex::new(r"\bcp\b").unwrap(),
            Regex::new(r"\bmkdir\b").unwrap(),
            Regex::new(r"\brmdir\b").unwrap(),
            Regex::new(r"\btouch\b").unwrap(),
            Regex::new(r"\bchmod\b").unwrap(),
            Regex::new(r"\bchown\b").unwrap(),
            Regex::new(r"\bln\b").unwrap(),
            Regex::new(r"\btar\b.*-[cxz]").unwrap(),
            Regex::new(r"\bunzip\b").unwrap(),
            Regex::new(r">\s*[^&]").unwrap(), // redirect to file
            Regex::new(r">>\s*").unwrap(),    // append to file
            Regex::new(r"\btee\b").unwrap(),
        ]
    })
}

#[allow(clippy::unwrap_used)]
fn get_network_patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            Regex::new(r"\bcurl\b").unwrap(),
            Regex::new(r"\bwget\b").unwrap(),
            Regex::new(r"\bnc\b").unwrap(),
            Regex::new(r"\bnetcat\b").unwrap(),
            Regex::new(r"\bssh\b").unwrap(),
            Regex::new(r"\bscp\b").unwrap(),
            Regex::new(r"\brsync\b").unwrap(),
            Regex::new(r"\bftp\b").unwrap(),
            Regex::new(r"\bsftp\b").unwrap(),
            Regex::new(r"\btelnet\b").unwrap(),
            Regex::new(r"\bping\b").unwrap(),
            Regex::new(r"\bnmap\b").unwrap(),
            Regex::new(r"\bnetstat\b").unwrap(),
        ]
    })
}

#[allow(clippy::unwrap_used)]
fn get_system_modify_patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            Regex::new(r"\bsudo\b").unwrap(),
            Regex::new(r"\bsu\s").unwrap(),
            Regex::new(r"\bsystemctl\b").unwrap(),
            Regex::new(r"\bservice\b").unwrap(),
            Regex::new(r"\bkill\b").unwrap(),
            Regex::new(r"\bpkill\b").unwrap(),
            Regex::new(r"\bkillall\b").unwrap(),
            Regex::new(r"\bshutdown\b").unwrap(),
            Regex::new(r"\breboot\b").unwrap(),
            Regex::new(r"\buseradd\b").unwrap(),
            Regex::new(r"\buserdel\b").unwrap(),
            Regex::new(r"\busermod\b").unwrap(),
            Regex::new(r"\bgroupadd\b").unwrap(),
            Regex::new(r"\bpasswd\b").unwrap(),
            Regex::new(r"\bmount\b").unwrap(),
            Regex::new(r"\bumount\b").unwrap(),
        ]
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_command() {
        let analyzer = CommandAnalyzer::restrictive();
        let result = analyzer.analyze("ls -la");
        assert_eq!(result.severity, Severity::Safe);
    }

    #[test]
    fn test_dangerous_command() {
        let analyzer = CommandAnalyzer::restrictive();
        let result = analyzer.analyze("rm -rf /tmp/test");
        assert!(result.severity >= Severity::Dangerous);
    }

    #[test]
    fn test_forbidden_command() {
        let analyzer = CommandAnalyzer::restrictive();
        let result = analyzer.analyze("rm -rf /");
        assert_eq!(result.severity, Severity::Forbidden);
    }

    #[test]
    fn test_filesystem_modification() {
        let analyzer = CommandAnalyzer::restrictive();
        let result = analyzer.analyze("touch file.txt");
        assert!(result.modifies_filesystem);
    }

    #[test]
    fn test_network_access() {
        let analyzer = CommandAnalyzer::restrictive();
        let result = analyzer.analyze("curl https://example.com");
        assert!(result.accesses_network);
    }

    #[test]
    fn test_system_modification() {
        let analyzer = CommandAnalyzer::restrictive();
        let result = analyzer.analyze("sudo apt update");
        assert!(result.modifies_system);
    }

    #[test]
    fn test_compound_command() {
        let analyzer = CommandAnalyzer::restrictive();
        let result = analyzer.analyze("ls && rm -rf /");
        assert_eq!(result.severity, Severity::Forbidden);
    }

    #[test]
    fn test_permissive_config() {
        let analyzer = CommandAnalyzer::permissive();
        let result = analyzer.analyze("rm -rf /tmp/test");
        // Permissive allows rm -rf in subdirs, just not root
        assert!(result.severity < Severity::Forbidden);
    }

    #[test]
    fn test_custom_safe_commands() {
        let config = SafetyConfig::restrictive().with_safe_commands(vec!["git".to_string()]);
        let analyzer = CommandAnalyzer::new(config);
        let result = analyzer.analyze("git status");
        assert_eq!(result.severity, Severity::Safe);
    }

    #[test]
    fn test_custom_forbidden_pattern() {
        let config = SafetyConfig::permissive().with_forbidden_patterns(vec![r"eval".to_string()]);
        let analyzer = CommandAnalyzer::new(config);
        let result = analyzer.analyze("eval '$dangerous'");
        assert_eq!(result.severity, Severity::Forbidden);
    }

    #[test]
    fn test_fork_bomb_detection() {
        let analyzer = CommandAnalyzer::restrictive();
        let result = analyzer.analyze(":(){ :|:& };:");
        assert_eq!(result.severity, Severity::Forbidden);
    }

    #[test]
    fn test_redirect_detection() {
        let analyzer = CommandAnalyzer::restrictive();
        let result = analyzer.analyze("echo test > file.txt");
        assert!(result.modifies_filesystem);
    }

    #[test]
    fn test_command_substitution() {
        let analyzer = CommandAnalyzer::restrictive();
        let result = analyzer.analyze("echo $(rm -rf /)");
        assert_eq!(result.severity, Severity::Forbidden);
    }
}
