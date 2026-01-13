//! Source file scanner for platform usage linter

use super::introspection::IntrospectionEnricher;
use super::patterns::{LintError, LintPattern, LintPatterns, LintResult, Severity};
use super::LintConfig;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// A single lint warning
#[derive(Debug, Clone, Serialize)]
pub struct LintWarning {
    /// File path where the warning was found
    pub file: PathBuf,

    /// Line number (1-indexed)
    pub line: usize,

    /// Column (1-indexed)
    pub column: usize,

    /// Pattern name that triggered
    pub pattern: String,

    /// Category of the pattern
    pub category: String,

    /// Severity level
    pub severity: Severity,

    /// The matched line content
    pub line_content: String,

    /// The trigger that matched
    pub trigger: String,

    /// User message
    pub message: String,

    /// Platform module to use instead
    pub platform_module: String,

    /// Example usage (if explain mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example_usage: Option<String>,

    /// Documentation URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs_url: Option<String>,

    /// Dynamically discovered alternative types from introspection
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub discovered_alternatives: Vec<DiscoveredAlternative>,
}

/// A dynamically discovered alternative type
#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredAlternative {
    /// Type name
    pub name: String,
    /// Full path
    pub path: String,
    /// Crate name
    pub crate_name: String,
    /// Type kind (struct, trait, fn, etc.)
    pub kind: String,
    /// Description
    pub description: String,
}

impl LintWarning {
    /// Format as a compiler-style warning
    pub fn format_text(&self, explain: bool) -> String {
        let mut output = format!(
            "{}: {}\n  --> {}:{}:{}\n   |\n{:>3} | {}\n   |\n   = DashFlow has: {}\n",
            self.severity.to_string().to_uppercase(),
            self.message,
            self.file.display(),
            self.line,
            self.column,
            self.line,
            self.line_content.trim(),
            self.platform_module,
        );

        // Show discovered alternatives (always shown, not just in explain mode)
        if !self.discovered_alternatives.is_empty() {
            output.push_str("\n   Discovered alternatives:\n");
            for alt in &self.discovered_alternatives {
                output.push_str(&format!(
                    "     - {} {} from {}\n",
                    alt.kind, alt.name, alt.crate_name
                ));
                if !alt.description.is_empty() && explain {
                    output.push_str(&format!("       {}\n", alt.description));
                }
            }
        }

        if explain {
            if let Some(ref example) = self.example_usage {
                output.push_str("\n   Example usage:\n");
                for line in example.lines() {
                    output.push_str(&format!("   | {}\n", line));
                }
            }

            if let Some(ref docs) = self.docs_url {
                output.push_str(&format!("\n   Docs: {}\n", docs));
            }
        }

        output.push_str(&format!(
            "\n   To suppress: Add `// dashflow-lint: ignore {}`\n",
            self.pattern
        ));

        output
    }
}

/// Result of linting a directory
#[derive(Debug, Clone, Serialize)]
pub struct ScanResult {
    /// Directory that was linted
    pub path: PathBuf,

    /// All warnings found
    pub warnings: Vec<LintWarning>,

    /// Files scanned
    pub files_scanned: usize,

    /// Lines scanned
    pub lines_scanned: usize,

    /// Warnings by severity
    #[serde(skip)]
    pub by_severity: HashMap<Severity, usize>,

    /// Warnings by pattern
    #[serde(skip)]
    pub by_pattern: HashMap<String, usize>,
}

impl ScanResult {
    /// Create a new empty result
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            warnings: Vec::new(),
            files_scanned: 0,
            lines_scanned: 0,
            by_severity: HashMap::new(),
            by_pattern: HashMap::new(),
        }
    }

    /// Add a warning
    pub fn add_warning(&mut self, warning: LintWarning) {
        *self.by_severity.entry(warning.severity).or_default() += 1;
        *self.by_pattern.entry(warning.pattern.clone()).or_default() += 1;
        self.warnings.push(warning);
    }

    /// Check if there are any errors
    pub fn has_errors(&self) -> bool {
        self.by_severity.get(&Severity::Error).unwrap_or(&0) > &0
    }

    /// Get total warning count
    pub fn warning_count(&self) -> usize {
        self.warnings.len()
    }

    /// Format summary
    pub fn format_summary(&self) -> String {
        let errors = self.by_severity.get(&Severity::Error).unwrap_or(&0);
        let warnings = self.by_severity.get(&Severity::Warn).unwrap_or(&0);
        let infos = self.by_severity.get(&Severity::Info).unwrap_or(&0);

        format!(
            "\nFound {} potential reimplementations ({} errors, {} warnings, {} info) in {} files ({} lines)\n",
            self.warnings.len(),
            errors,
            warnings,
            infos,
            self.files_scanned,
            self.lines_scanned,
        )
    }

    /// Output as text
    pub fn to_text(&self, explain: bool) -> String {
        let mut output = String::new();

        for warning in &self.warnings {
            output.push_str(&warning.format_text(explain));
            output.push('\n');
        }

        output.push_str(&self.format_summary());

        if !self.warnings.is_empty() {
            output.push_str("Run `dashflow lint --explain` for detailed suggestions.\n");
        }

        output
    }

    /// Output as JSON
    pub fn to_json(&self) -> LintResult<String> {
        serde_json::to_string_pretty(self).map_err(LintError::from)
    }

    /// Output as SARIF format for IDE integration (VS Code, IntelliJ, GitHub)
    ///
    /// SARIF (Static Analysis Results Interchange Format) is a standard JSON format
    /// supported by many IDEs and CI systems.
    pub fn to_sarif(&self) -> LintResult<String> {
        let sarif = SarifReport::from_scan_result(self);
        serde_json::to_string_pretty(&sarif).map_err(LintError::from)
    }
}

/// SARIF 2.1.0 compliant report structure
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifReport {
    #[serde(rename = "$schema")]
    schema: &'static str,
    version: &'static str,
    runs: Vec<SarifRun>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRun {
    tool: SarifTool,
    results: Vec<SarifResult>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifTool {
    driver: SarifDriver,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifDriver {
    name: &'static str,
    version: &'static str,
    information_uri: &'static str,
    rules: Vec<SarifRule>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRule {
    id: String,
    name: String,
    short_description: SarifMessage,
    full_description: SarifMessage,
    help_uri: Option<String>,
    default_configuration: SarifRuleConfiguration,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRuleConfiguration {
    level: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifResult {
    rule_id: String,
    level: &'static str,
    message: SarifMessage,
    locations: Vec<SarifLocation>,
}

#[derive(Debug, Clone, Serialize)]
struct SarifMessage {
    text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifLocation {
    physical_location: SarifPhysicalLocation,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifPhysicalLocation {
    artifact_location: SarifArtifactLocation,
    region: SarifRegion,
}

#[derive(Debug, Clone, Serialize)]
struct SarifArtifactLocation {
    uri: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRegion {
    start_line: usize,
    start_column: usize,
    snippet: Option<SarifSnippet>,
}

#[derive(Debug, Clone, Serialize)]
struct SarifSnippet {
    text: String,
}

impl SarifReport {
    fn from_scan_result(result: &ScanResult) -> Self {
        // Collect unique rules from warnings
        let mut rules_map: HashMap<String, SarifRule> = HashMap::new();
        for warning in &result.warnings {
            if !rules_map.contains_key(&warning.pattern) {
                rules_map.insert(
                    warning.pattern.clone(),
                    SarifRule {
                        id: warning.pattern.clone(),
                        name: warning.pattern.replace('_', " ").to_string(),
                        short_description: SarifMessage {
                            text: warning.message.clone(),
                        },
                        full_description: SarifMessage {
                            text: format!(
                                "{} Consider using: {}",
                                warning.message, warning.platform_module
                            ),
                        },
                        help_uri: warning.docs_url.clone(),
                        default_configuration: SarifRuleConfiguration {
                            level: match warning.severity {
                                Severity::Info => "note",
                                Severity::Warn => "warning",
                                Severity::Error => "error",
                            },
                        },
                    },
                );
            }
        }

        let results: Vec<SarifResult> = result
            .warnings
            .iter()
            .map(|w| SarifResult {
                rule_id: w.pattern.clone(),
                level: match w.severity {
                    Severity::Info => "note",
                    Severity::Warn => "warning",
                    Severity::Error => "error",
                },
                message: SarifMessage {
                    text: format!("{} Use {} instead.", w.message, w.platform_module),
                },
                locations: vec![SarifLocation {
                    physical_location: SarifPhysicalLocation {
                        artifact_location: SarifArtifactLocation {
                            uri: w.file.to_string_lossy().to_string(),
                        },
                        region: SarifRegion {
                            start_line: w.line,
                            start_column: w.column,
                            snippet: Some(SarifSnippet {
                                text: w.line_content.clone(),
                            }),
                        },
                    },
                }],
            })
            .collect();

        SarifReport {
            schema: "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json",
            version: "2.1.0",
            runs: vec![SarifRun {
                tool: SarifTool {
                    driver: SarifDriver {
                        name: "dashflow-lint",
                        version: env!("CARGO_PKG_VERSION"),
                        information_uri: "https://github.com/dropbox/dTOOL/dashflow",
                        rules: rules_map.into_values().collect(),
                    },
                },
                results,
            }],
        }
    }
}

/// Scanner for finding pattern matches in source files
pub struct LintScanner {
    patterns: LintPatterns,
    config: LintConfig,
    enricher: Option<IntrospectionEnricher>,
}

impl LintScanner {
    /// Create a new scanner with patterns and config
    pub fn new(patterns: LintPatterns, config: LintConfig) -> Self {
        Self {
            patterns,
            config,
            enricher: None,
        }
    }

    /// Create a scanner with introspection enabled
    #[must_use]
    pub fn with_introspection(mut self, workspace_root: PathBuf) -> Self {
        self.enricher = Some(IntrospectionEnricher::new(workspace_root));
        self
    }

    /// Scan a directory for pattern matches
    ///
    /// This wraps the blocking directory walk and file reads in `spawn_blocking`
    /// to avoid blocking the async runtime.
    pub async fn scan_directory(&self, path: &Path) -> LintResult<ScanResult> {
        // Clone what we need for the blocking task
        let path = path.to_path_buf();
        let config = self.config.clone();
        let patterns = self.patterns.clone();
        let enricher = self.enricher.clone();

        // Run blocking I/O in spawn_blocking
        tokio::task::spawn_blocking(move || {
            let scanner = LintScanner {
                patterns,
                config,
                enricher,
            };
            scanner.scan_directory_sync(&path)
        })
        .await
        .map_err(|e| LintError::Other(format!("spawn_blocking panicked: {e}")))?
    }

    /// Synchronous directory scan (for use in spawn_blocking)
    fn scan_directory_sync(&self, path: &Path) -> LintResult<ScanResult> {
        let mut result = ScanResult::new(path.to_path_buf());

        let walker = WalkDir::new(path)
            .follow_links(self.config.follow_symlinks)
            .into_iter()
            .filter_entry(|e| !self.should_skip(e.path()));

        for entry in walker.filter_map(|e| e.ok()) {
            let path = entry.path();

            // Only scan Rust files
            if path.extension().map_or(true, |ext| ext != "rs") {
                continue;
            }

            // Skip excluded paths
            if self.is_excluded(path) {
                continue;
            }

            self.scan_file(path, &mut result)?;
        }

        Ok(result)
    }

    /// Scan a single file for pattern matches
    ///
    /// This wraps the blocking file read in `spawn_blocking`
    /// to avoid blocking the async runtime.
    pub async fn scan_single_file(&self, path: &Path) -> LintResult<ScanResult> {
        // Only scan Rust files
        if path.extension().map_or(true, |ext| ext != "rs") {
            return Ok(ScanResult::new(path.to_path_buf()));
        }

        // Clone what we need for the blocking task
        let path = path.to_path_buf();
        let config = self.config.clone();
        let patterns = self.patterns.clone();
        let enricher = self.enricher.clone();

        // Run blocking I/O in spawn_blocking
        tokio::task::spawn_blocking(move || {
            let scanner = LintScanner {
                patterns,
                config,
                enricher,
            };
            let mut result = ScanResult::new(path.clone());
            scanner.scan_file(&path, &mut result)?;
            Ok(result)
        })
        .await
        .map_err(|e| LintError::Other(format!("spawn_blocking panicked: {e}")))?
    }

    /// Check if a path should be skipped entirely
    fn should_skip(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        // Skip common non-source directories
        if path_str.contains("/target/")
            || path_str.contains("/.git/")
            || path_str.contains("/node_modules/")
        {
            return true;
        }

        false
    }

    /// Check if a path is excluded by config
    fn is_excluded(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        for exclude in &self.config.exclude_paths {
            if path_str.contains(exclude) {
                return true;
            }
        }

        false
    }

    /// Scan a single file
    fn scan_file(&self, path: &Path, result: &mut ScanResult) -> LintResult<()> {
        let content = std::fs::read_to_string(path).map_err(|source| LintError::FileRead {
            path: path.display().to_string(),
            source,
        })?;

        result.files_scanned += 1;

        let mut in_ignore_block = false;
        let mut ignore_patterns: Vec<String> = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            result.lines_scanned += 1;

            // Check for ignore comments
            if let Some(ignore_comment) = self.parse_ignore_comment(line) {
                match ignore_comment {
                    IgnoreDirective::Line(patterns) => {
                        // Ignore only this line for specified patterns
                        ignore_patterns = patterns;
                    }
                    IgnoreDirective::Begin(patterns) => {
                        in_ignore_block = true;
                        ignore_patterns = patterns;
                        continue;
                    }
                    IgnoreDirective::End => {
                        in_ignore_block = false;
                        ignore_patterns.clear();
                        continue;
                    }
                }
            }

            // Skip if in ignore block
            if in_ignore_block {
                continue;
            }

            // Check all patterns
            for pattern in self.patterns.iter() {
                // Skip if severity below threshold
                if pattern.severity < self.config.min_severity {
                    continue;
                }

                // Skip if pattern is excluded for this path
                if pattern.is_excluded(path) {
                    continue;
                }

                // Skip if pattern is in ignore list
                if ignore_patterns.contains(&pattern.name)
                    || ignore_patterns.iter().any(|p| p == "*")
                {
                    continue;
                }

                // Check for match
                if let Some(trigger) = pattern.matches(line) {
                    let warning = self.create_warning(path, line_num + 1, line, pattern, trigger);
                    result.add_warning(warning);
                }
            }

            // Clear line-level ignores
            ignore_patterns.clear();
        }

        Ok(())
    }

    /// Parse an ignore comment directive
    fn parse_ignore_comment(&self, line: &str) -> Option<IgnoreDirective> {
        let trimmed = line.trim();

        // Check for dashflow-lint: ignore-begin (MUST check before bare "ignore")
        if let Some(rest) = trimmed.strip_prefix("// dashflow-lint: ignore-begin") {
            let patterns: Vec<String> = rest
                .trim()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            let patterns = if patterns.is_empty() {
                vec!["*".to_string()]
            } else {
                patterns
            };

            return Some(IgnoreDirective::Begin(patterns));
        }

        // Check for dashflow-lint: ignore-end (MUST check before bare "ignore")
        if trimmed.starts_with("// dashflow-lint: ignore-end") {
            return Some(IgnoreDirective::End);
        }

        // Check for dashflow-lint: ignore (single line, check AFTER ignore-begin/end)
        if let Some(rest) = trimmed.strip_prefix("// dashflow-lint: ignore") {
            let patterns: Vec<String> = rest
                .trim()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            // Empty means ignore all on this line
            let patterns = if patterns.is_empty() {
                vec!["*".to_string()]
            } else {
                patterns
            };

            return Some(IgnoreDirective::Line(patterns));
        }

        None
    }

    /// Create a warning from a match
    fn create_warning(
        &self,
        path: &Path,
        line: usize,
        line_content: &str,
        pattern: &LintPattern,
        trigger: &str,
    ) -> LintWarning {
        // Find column where match starts
        let column = line_content
            .find(
                trigger
                    .trim_start_matches("fn\\s+")
                    .trim_start_matches("struct\\s+"),
            )
            .unwrap_or(0)
            + 1;

        // Discover alternatives via introspection if available
        let discovered_alternatives = if let Some(ref enricher) = self.enricher {
            enricher
                .find_alternatives(&pattern.name, &pattern.category)
                .into_iter()
                .map(|alt| DiscoveredAlternative {
                    name: alt.name,
                    path: alt.path,
                    crate_name: alt.crate_name,
                    kind: alt.kind.to_string(),
                    description: alt.description,
                })
                .collect()
        } else {
            Vec::new()
        };

        LintWarning {
            file: path.to_path_buf(),
            line,
            column,
            pattern: pattern.name.clone(),
            category: pattern.category.clone(),
            severity: pattern.severity,
            line_content: line_content.to_string(),
            trigger: trigger.to_string(),
            message: pattern.message.clone(),
            platform_module: pattern.platform_module.clone(),
            example_usage: if self.config.explain {
                pattern.example_usage.clone()
            } else {
                None
            },
            docs_url: pattern.docs_url.clone(),
            discovered_alternatives,
        }
    }
}

/// Ignore directive types
enum IgnoreDirective {
    /// Ignore patterns for the next line
    Line(Vec<String>),
    /// Begin ignore block
    Begin(Vec<String>),
    /// End ignore block
    End,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ignore_parsing() {
        let patterns = LintPatterns::load_default().unwrap();
        let scanner = LintScanner::new(patterns, LintConfig::default());

        // Test line ignore
        let directive = scanner.parse_ignore_comment("// dashflow-lint: ignore cost_tracking");
        assert!(matches!(directive, Some(IgnoreDirective::Line(_))));

        // Test ignore all
        let directive = scanner.parse_ignore_comment("// dashflow-lint: ignore");
        if let Some(IgnoreDirective::Line(patterns)) = directive {
            assert_eq!(patterns, vec!["*"]);
        } else {
            panic!("Expected Line directive");
        }

        // Test block ignore
        let directive =
            scanner.parse_ignore_comment("// dashflow-lint: ignore-begin cost_tracking, bm25");
        assert!(matches!(directive, Some(IgnoreDirective::Begin(_))));
    }

    #[test]
    fn test_warning_format() {
        let warning = LintWarning {
            file: PathBuf::from("src/cost.rs"),
            line: 15,
            column: 1,
            pattern: "cost_tracking".to_string(),
            category: "observability".to_string(),
            severity: Severity::Warn,
            line_content: "pub struct CostTracker {".to_string(),
            trigger: "struct\\s+CostTracker".to_string(),
            message: "DashFlow has built-in cost tracking".to_string(),
            platform_module: "dashflow_observability::cost".to_string(),
            example_usage: None,
            docs_url: None,
            discovered_alternatives: Vec::new(),
        };

        let output = warning.format_text(false);
        assert!(output.contains("WARNING"));
        assert!(output.contains("src/cost.rs:15:1"));
        assert!(output.contains("CostTracker"));
        assert!(output.contains("dashflow_observability::cost"));
    }

    #[test]
    fn test_result_summary() {
        let mut result = ScanResult::new(PathBuf::from("test"));
        result.files_scanned = 10;
        result.lines_scanned = 500;

        let summary = result.format_summary();
        assert!(summary.contains("0 potential reimplementations"));
        assert!(summary.contains("10 files"));
        assert!(summary.contains("500 lines"));
    }
}
