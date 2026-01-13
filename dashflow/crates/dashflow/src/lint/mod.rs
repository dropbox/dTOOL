//! Platform Usage Linter
//!
//! Detects potential reimplementations of DashFlow platform features in app code.
//! Helps developers (human or AI) discover existing platform functionality.
//!
//! # Usage
//!
//! ```bash
//! dashflow lint examples/apps/librarian
//! dashflow lint --explain src/
//! dashflow lint --json .
//! ```
//!
//! # Pattern Sources
//!
//! The linter supports two pattern sources:
//!
//! 1. **Registry patterns** (default): Dynamic patterns from `ModulePatternRegistry`
//!    populated via `#[dashflow::capability(...)]` proc macro attributes. This is the
//!    introspection-powered approach with richer metadata.
//! 2. **YAML patterns**: Static patterns defined in `lint/patterns.yaml`. Use
//!    `LintConfig::with_use_registry(false)` or the `--use-yaml` CLI flag.
//!
//! The registry approach is the default because it provides up-to-date patterns
//! that automatically reflect platform capabilities via proc macro annotations.

mod feedback;
mod introspection;
mod pattern_generator;
mod patterns;
mod scanner;
mod semantic;
mod telemetry;

pub use feedback::{
    submit_feedback, FeedbackCollector, FeedbackEntry, FeedbackStore, PatternStats,
};
pub use introspection::{AlternativeType, IntrospectionEnricher, TypeIndex, TypeIndexCache};
pub use pattern_generator::{GeneratedPattern, PatternGenerator};
pub use patterns::{LintError, LintPattern, LintPatterns, LintResult, PatternMatch, Severity};
pub use scanner::{DiscoveredAlternative, LintScanner, LintWarning, ScanResult};
pub use semantic::{SemanticIndex, SimilarityResult};
pub use telemetry::{
    send_report, ReportDestination, TelemetryCollector, TelemetryConfig, TelemetryReport,
};

use crate::introspection::ModulePatternRegistry;
use std::path::Path;

/// Load patterns based on configuration.
///
/// Pattern loading has two modes:
///
/// 1. **Registry mode** (default, `config.use_registry = true`):
///    Uses `ModulePatternRegistry::with_defaults()` which contains patterns
///    populated via `#[dashflow::capability(...)]` proc macro attributes.
///    This is the introspection-based approach.
///
/// 2. **YAML mode** (`config.use_registry = false`):
///    Loads patterns from `lint/patterns.yaml`, optionally merging with
///    dynamically generated patterns from the type index.
fn load_patterns_with_generation(config: &LintConfig) -> LintResult<LintPatterns> {
    // Use registry-based patterns if configured
    if config.use_registry {
        let registry = ModulePatternRegistry::with_defaults();
        return LintPatterns::from_registry(&registry);
    }

    // Default: Start with static YAML patterns (and workspace overrides if present)
    let static_patterns = if let Some(ref workspace_root) = config.workspace_root {
        LintPatterns::load_with_workspace(workspace_root)?
    } else {
        LintPatterns::load_default()?
    };

    // If introspection is enabled, merge with generated patterns
    if let Some(ref workspace_root) = config.workspace_root {
        let type_index = TypeIndex::build(workspace_root.clone());
        let generator = PatternGenerator::new(&type_index);

        // Merge generated patterns with static (static takes priority)
        generator.merge_with_static(&static_patterns)
    } else {
        Ok(static_patterns)
    }
}

/// Run the platform usage linter on a directory
pub async fn lint_directory(path: &Path, config: LintConfig) -> LintResult<ScanResult> {
    let patterns = load_patterns_with_generation(&config)?;
    let mut scanner = LintScanner::new(patterns, config.clone());

    // Enable introspection if workspace root is configured
    if let Some(ref workspace_root) = config.workspace_root {
        scanner = scanner.with_introspection(workspace_root.clone());
    }

    scanner.scan_directory(path).await
}

/// Run the platform usage linter on a single file
pub async fn lint_file(path: &Path, config: LintConfig) -> LintResult<ScanResult> {
    let patterns = load_patterns_with_generation(&config)?;
    let mut scanner = LintScanner::new(patterns, config.clone());

    // Enable introspection if workspace root is configured
    if let Some(ref workspace_root) = config.workspace_root {
        scanner = scanner.with_introspection(workspace_root.clone());
    }

    scanner.scan_single_file(path).await
}

/// Run the platform usage linter on a path (file or directory)
///
/// Automatically detects whether the path is a file or directory and calls
/// the appropriate function.
pub async fn lint_path(path: &Path, config: LintConfig) -> LintResult<ScanResult> {
    if path.is_file() {
        lint_file(path, config).await
    } else {
        lint_directory(path, config).await
    }
}

/// Configuration for the linter
#[derive(Debug, Clone)]
pub struct LintConfig {
    /// Show detailed explanations for warnings
    pub explain: bool,

    /// Output format (text, json)
    pub format: OutputFormat,

    /// Minimum severity level to report
    pub min_severity: Severity,

    /// Additional paths to exclude
    pub exclude_paths: Vec<String>,

    /// Whether to follow symlinks
    pub follow_symlinks: bool,

    /// Workspace root for introspection (enables dynamic alternative discovery)
    pub workspace_root: Option<std::path::PathBuf>,

    /// Enable telemetry collection for this lint run
    pub enable_telemetry: bool,

    /// Use `ModulePatternRegistry` instead of YAML patterns.
    ///
    /// When enabled (default), patterns are loaded from the introspection-based
    /// registry populated via `#[dashflow::capability(...)]` attributes.
    /// Set to `false` (or use `--use-yaml` CLI flag) to use static YAML patterns.
    pub use_registry: bool,
}

impl Default for LintConfig {
    fn default() -> Self {
        Self {
            explain: false,
            format: OutputFormat::default(),
            min_severity: Severity::default(),
            exclude_paths: Vec::new(),
            follow_symlinks: false,
            workspace_root: None,
            enable_telemetry: false,
            use_registry: true, // Registry patterns are the default
        }
    }
}

/// Output format for lint results
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable text output.
    #[default]
    Text,
    /// JSON output for programmatic consumption.
    Json,
    /// SARIF format for IDE integration (VS Code, IntelliJ, GitHub).
    Sarif,
}

impl LintConfig {
    /// Create a new lint configuration with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable detailed explanations
    #[must_use]
    pub fn with_explain(mut self, explain: bool) -> Self {
        self.explain = explain;
        self
    }

    /// Set output format
    #[must_use]
    pub fn with_format(mut self, format: OutputFormat) -> Self {
        self.format = format;
        self
    }

    /// Set minimum severity
    #[must_use]
    pub fn with_min_severity(mut self, severity: Severity) -> Self {
        self.min_severity = severity;
        self
    }

    /// Add excluded paths
    #[must_use]
    pub fn with_excludes(mut self, paths: Vec<String>) -> Self {
        self.exclude_paths = paths;
        self
    }

    /// Set workspace root for introspection-based alternative discovery
    #[must_use]
    pub fn with_workspace_root(mut self, workspace_root: std::path::PathBuf) -> Self {
        self.workspace_root = Some(workspace_root);
        self
    }

    /// Enable telemetry collection for this lint run
    #[must_use]
    pub fn with_telemetry(mut self, enable: bool) -> Self {
        self.enable_telemetry = enable;
        self
    }

    /// Control pattern source: registry (default) or YAML.
    ///
    /// When `true` (default), patterns are loaded from the introspection-based
    /// registry populated via `#[dashflow::capability(...)]` attributes.
    /// When `false`, patterns are loaded from static YAML files.
    #[must_use]
    pub fn with_use_registry(mut self, use_registry: bool) -> Self {
        self.use_registry = use_registry;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lint_config_builder() {
        let config = LintConfig::new()
            .with_explain(true)
            .with_format(OutputFormat::Json)
            .with_min_severity(Severity::Warn);

        assert!(config.explain);
        assert_eq!(config.format, OutputFormat::Json);
        assert_eq!(config.min_severity, Severity::Warn);
    }

    #[test]
    fn test_lint_config_use_registry_default() {
        // Registry is now the default
        let config = LintConfig::new();
        assert!(config.use_registry, "Registry should be enabled by default");
        assert!(!config.explain);

        // Can be disabled explicitly
        let config = LintConfig::new().with_use_registry(false);
        assert!(!config.use_registry);
    }

    #[test]
    fn test_load_patterns_from_registry() {
        // Registry is the default, so no explicit with_use_registry(true) needed
        let config = LintConfig::new();
        let patterns = load_patterns_with_generation(&config).unwrap();

        // Registry has 17 patterns
        assert_eq!(patterns.patterns.len(), 17);

        // Check categories are mapped correctly
        let categories = patterns.categories();
        assert!(categories.contains(&"observability"));
        assert!(categories.contains(&"retrievers"));
        assert!(categories.contains(&"evaluation"));
        assert!(categories.contains(&"models"));
    }

    #[test]
    fn test_registry_patterns_match_yaml_patterns() {
        // Load from YAML
        let yaml_patterns = LintPatterns::load_default().unwrap();

        // Load from registry
        let registry = ModulePatternRegistry::with_defaults();
        let registry_patterns = LintPatterns::from_registry(&registry).unwrap();

        // Both should detect the same patterns (cost tracking)
        let test_line = "pub struct CostTracker {";

        let yaml_matches: Vec<_> = yaml_patterns
            .iter()
            .filter(|p| p.matches(test_line).is_some())
            .collect();

        let registry_matches: Vec<_> = registry_patterns
            .iter()
            .filter(|p| p.matches(test_line).is_some())
            .collect();

        assert!(!yaml_matches.is_empty(), "YAML patterns should match CostTracker");
        assert!(
            !registry_matches.is_empty(),
            "Registry patterns should match CostTracker"
        );
    }
}
