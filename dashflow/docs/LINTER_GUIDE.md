# Platform Usage Linter Guide

**Version:** 1.11
**Last Updated:** 2026-01-02 (Worker #2337 - Add missing Last Updated headers)

The DashFlow Platform Usage Linter helps developers (human or AI) discover existing platform features they might be reimplementing. It scans source code for patterns that suggest custom implementations of functionality that DashFlow already provides.

## Quick Start

```bash
# Lint the current directory (uses registry patterns by default)
dashflow lint .

# Lint a specific directory with explanations
dashflow lint --explain examples/apps/librarian

# Use static YAML patterns instead of registry (for stable CI runs)
dashflow lint --use-yaml src/

# Output in JSON format
dashflow lint --format json src/

# Output in SARIF format for IDE integration
dashflow lint --format sarif src/ > lint-results.sarif
```

## Why Use the Linter?

DashFlow provides 100+ production-ready modules for AI applications:

- **Retrievers**: BM25, semantic, hybrid search
- **Cost Tracking**: Token counting, budget enforcement, usage analytics
- **Evaluation**: Test suites, scoring methods, metrics
- **Observability**: Tracing, metrics, logging
- **And more...**

When building applications, it's easy to miss existing platform features and reimplement them from scratch. The linter catches these cases and points you to the right platform modules.

## Usage

### Basic Scanning

```bash
# Scan current directory
dashflow lint

# Scan specific path (file or directory)
dashflow lint src/search.rs
dashflow lint examples/apps/librarian/
```

### Output Formats

```bash
# Human-readable text (default)
dashflow lint --format text

# JSON for automation
dashflow lint --format json > results.json

# SARIF for IDE integration (VS Code, IntelliJ, GitHub)
dashflow lint --format sarif > results.sarif
```

### Severity Filtering

```bash
# Show all messages including info
dashflow lint --severity info

# Show warnings and errors (default)
dashflow lint --severity warn

# Show only errors
dashflow lint --severity error
```

### Detailed Explanations

```bash
# Show example usage for each warning
dashflow lint --explain src/

# Short flag
dashflow lint -e src/
```

## Suppressing Warnings

### Line-Level Suppression

```rust
// Suppress a specific pattern on the next line
// dashflow-lint: ignore cost_tracking
pub struct CostTracker { /* ... */ }

// Suppress multiple patterns
// dashflow-lint: ignore cost_tracking, bm25_search
fn search_with_cost() { /* ... */ }

// Suppress all patterns on this line
// dashflow-lint: ignore
pub fn custom_implementation() { /* ... */ }
```

### Block-Level Suppression

```rust
// dashflow-lint: ignore-begin cost_tracking
pub struct CostTracker {
    // ... custom implementation justified for specific reason
}

impl CostTracker {
    // ... methods
}
// dashflow-lint: ignore-end
```

## Providing Feedback

When you decide not to use a platform feature, you can provide feedback explaining why. This helps the DashFlow team understand gaps in the platform.

### Inline Feedback

```bash
# Submit feedback with lint run
dashflow lint --feedback "Platform CostTracker doesn't support per-query breakdown" src/
```

### Managing Feedback

```bash
# List all collected feedback
dashflow lint feedback list

# Show only unreviewed feedback
dashflow lint feedback list --unreviewed

# Filter by pattern
dashflow lint feedback list --pattern cost_tracking

# Show summary statistics
dashflow lint feedback summary

# Export feedback to file
dashflow lint feedback export --output feedback.json

# Mark feedback as reviewed
dashflow lint feedback review <feedback-id>

# Submit feedback manually
dashflow lint feedback submit --pattern cost_tracking "Reason for not using platform"
```

## Pattern Categories

The linter detects patterns in these categories:

### Observability
- `cost_tracking` - Custom token/API cost tracking
- `metrics` - Custom metrics collection
- `tracing` - Custom trace collection

### Retrieval
- `bm25_search` - Custom BM25/keyword search
- `semantic_search` - Custom vector search
- `hybrid_search` - Custom combined search

### Evaluation
- `eval_framework` - Custom evaluation harness
- `eval_metrics` - Custom evaluation metrics
- `scoring` - Custom answer scoring

### LLM Integration
- `llm_wrapper` - Custom LLM API wrappers
- `embedding_wrapper` - Custom embedding wrappers
- `chat_model` - Custom chat model implementations

## Example Output

```
WARNING: DashFlow has built-in cost tracking
  --> examples/apps/librarian/src/cost.rs:52:1
   |
52 | pub struct CostTracker {
   |
   = DashFlow has: dashflow_observability::cost

   Discovered alternatives:
     - struct CostTracker from dashflow_observability
       Per-token and per-request cost tracking with budget enforcement

   To suppress: Add `// dashflow-lint: ignore cost_tracking`

Found 3 potential reimplementations (0 errors, 3 warnings, 0 info) in 47 files (5823 lines)
Run `dashflow lint --explain` for detailed suggestions.
```

## IDE Integration

### VS Code with SARIF

1. Install the SARIF Viewer extension
2. Run: `dashflow lint --format sarif > .dashflow/lint.sarif`
3. Open the SARIF file in VS Code to see inline annotations

### Cargo Workflow Scripts

DashFlow provides convenience scripts for integrating linting into your development workflow:

```bash
# Run lint standalone
./scripts/cargo_lint.sh examples/apps/librarian

# Comprehensive check: cargo check + clippy + lint
./scripts/cargo_check_lint.sh

# Quick check without lint (faster)
./scripts/cargo_check_lint.sh --quick

# Lint only, skip cargo checks
./scripts/cargo_check_lint.sh --lint-only

# Strict mode: treat warnings as errors
./scripts/cargo_check_lint.sh --strict
```

Note: `./scripts/cargo_check_lint.sh` always enforces M-294 for production targets (no `unwrap()`/`expect()` in `--lib --bins`). If intentional, allow locally with `#[allow(clippy::unwrap_used|expect_used)]` and a SAFETY justification comment.

### Pre-commit Hook

Add to `.git/hooks/pre-commit`:

```bash
#!/bin/bash
# Lint changed Rust files before commit
changed_files=$(git diff --cached --name-only --diff-filter=ACMR | grep '\.rs$')
if [ -n "$changed_files" ]; then
    dashflow lint --severity error $changed_files || exit 1
fi
```

## CI Integration

> **Note:** DashFlow uses internal Dropbox CI, not GitHub Actions. The `.github/` directory does not exist in this repository. The examples below are templates for teams using GitHub Actions.

### GitHub Actions (Example)

```yaml
- name: Run Platform Usage Linter
  run: |
    dashflow lint --format sarif --severity warn > results.sarif

- name: Upload SARIF
  uses: github/codeql-action/upload-sarif@v2
  with:
    sarif_file: results.sarif
```

## Configuration

### Exclude Paths

```bash
# Exclude specific directories
dashflow lint --exclude tests --exclude benches .

# Multiple exclusions
dashflow lint -e test_utils -e fixtures src/
```

### Follow Symlinks

```bash
# Follow symbolic links (disabled by default)
dashflow lint --follow-symlinks .
```

### Pattern Source Selection

The linter can load patterns from two sources:

1. **Registry patterns (default)**: Dynamic patterns from `ModulePatternRegistry`, populated via `#[dashflow::capability(...)]` proc macro attributes on platform modules. Provides richer metadata and enables introspection-powered self-linting that automatically reflects platform capabilities.

2. **YAML patterns (`--use-yaml`)**: Static patterns defined in `lint/patterns.yaml`. Good for stability and explicit control in CI environments.

```bash
# Use default registry patterns (introspection-powered)
dashflow lint src/

# Registry patterns with explanations
dashflow lint --explain src/

# Use static YAML patterns for reproducible CI runs
dashflow lint --use-yaml src/
```

**When to use default registry patterns:**
- When you want patterns to automatically reflect the latest platform capabilities
- For introspection-powered suggestions with module metadata from proc macros
- When developing new platform modules with `#[dashflow::capability(...)]` annotations

**When to use `--use-yaml`:**
- For stable, reproducible CI runs where pattern changes shouldn't break builds
- When you need explicit control over which patterns are active
- For custom patterns specific to your project (see Custom Patterns section below)

## Introspection Integration

The linter automatically uses DashFlow's introspection system to:

1. **Discover alternatives**: Find platform types that match the pattern
2. **Show live documentation**: Extract descriptions from actual source code
3. **Suggest migrations**: Provide concrete example usage

This means lint suggestions are always up-to-date with the actual platform capabilities.

## Custom Patterns

You can add custom patterns in your workspace:

```bash
# Create .dashflow/lint/patterns.yaml in your project
mkdir -p .dashflow/lint
```

Example `.dashflow/lint/patterns.yaml`:

```yaml
patterns:
  - name: custom_cache
    category: caching
    triggers:
      - "struct\\s+CustomCache"
      - "fn\\s+cache_result"
    platform_module: "dashflow::caching::Cache"
    message: "Use DashFlow's built-in caching system"
    severity: warn
    example_usage: |
      use dashflow::caching::Cache;
      let cache = Cache::new().with_ttl(Duration::from_secs(300));
```

## Feedback Collection

Feedback is stored in `.dashflow/feedback/lint_feedback.json`. This file contains:

```json
{
  "entries": [
    {
      "id": "1734567890-cost_tracking-abc123",
      "timestamp": "2025-12-18T12:00:00Z",
      "pattern": "cost_tracking",
      "category": "observability",
      "file": "src/cost.rs",
      "line": 52,
      "reason": "Platform doesn't support per-query breakdown by search mode",
      "suggested_enhancement": "Add mode-specific cost aggregation",
      "reporter": "ai-worker-1022",
      "platform_module": "dashflow_observability::cost",
      "reviewed": false
    }
  ],
  "stats_by_pattern": {
    "cost_tracking": {
      "count": 5,
      "enhancement_suggestions": 2
    }
  }
}
```

## API Usage

You can also use the linter programmatically:

```rust
use dashflow::lint::{lint_directory, LintConfig, OutputFormat, Severity};
use std::path::Path;

async fn check_code() -> anyhow::Result<()> {
    let config = LintConfig::new()
        .with_explain(true)
        .with_format(OutputFormat::Json)
        .with_min_severity(Severity::Warn);

    let result = lint_directory(Path::new("src/"), config).await?;

    if result.has_errors() {
        eprintln!("{}", result.to_text(true));
    }

    Ok(())
}
```

## Troubleshooting

### "No patterns found"

Ensure you're running from within a DashFlow workspace, or specify the workspace root:

```bash
cd /path/to/dashflow-workspace
dashflow lint .
```

### "Pattern not matching expected code"

Patterns use regex. Check the pattern definition:

```bash
# View pattern details
dashflow lint --explain . 2>&1 | grep -A10 "your_pattern_name"
```

### "Too many warnings"

Filter by severity or add suppressions:

```bash
# Show only errors
dashflow lint --severity error .

# Or add block suppressions for intentional reimplementations
```

## Telemetry (Opt-In)

DashFlow can collect anonymous, aggregated telemetry about lint patterns to help prioritize platform improvements. **Telemetry is disabled by default** and requires explicit opt-in.

### Privacy Guarantees

- **No source code** is ever transmitted
- **No file paths** are included
- Only **aggregated pattern counts** and **classified feedback themes** are collected
- **Anonymous installation ID** - cannot be traced to individuals
- **Minimum threshold** - data is only sent after 10+ lint runs to ensure anonymization

### Managing Telemetry

```bash
# Check telemetry status
dashflow lint telemetry status

# Enable telemetry (opt-in)
dashflow lint telemetry enable

# Disable telemetry
dashflow lint telemetry disable

# Preview what data would be sent (without sending)
dashflow lint telemetry preview

# Send accumulated report
dashflow lint telemetry send

# Clear all accumulated data
dashflow lint telemetry clear
```

### Per-Run Telemetry

You can also enable telemetry for individual lint runs:

```bash
# Enable telemetry for this run only
dashflow lint --enable-telemetry src/

# Or set environment variable
DASHFLOW_LINT_TELEMETRY=1 dashflow lint src/
```

### What Gets Collected

```json
{
  "version": "1.0",
  "installation_id": "anon-abc123...",
  "lint_runs": 15,
  "pattern_counts": {
    "cost_tracking": {
      "match_count": 3,
      "suppression_count": 1,
      "feedback_count": 2,
      "feedback_categories": {
        "missing_feature": 1,
        "api_mismatch": 1
      }
    }
  },
  "feedback_summary": {
    "total_entries": 5,
    "enhancement_suggestions": 2,
    "themes": {
      "missing_feature": 3,
      "api_mismatch": 2
    }
  }
}
```

## CI Integration for Example Apps

DashFlow provides a dedicated script for linting all example applications:

```bash
# Lint all example apps
./scripts/lint_example_apps.sh

# Lint with strict mode (fail on warnings)
./scripts/lint_example_apps.sh --strict

# JSON output for CI parsing
./scripts/lint_example_apps.sh --json > lint-results.json

# SARIF output for GitHub/IDE integration
./scripts/lint_example_apps.sh --sarif > lint-results.sarif

# Lint specific app only
./scripts/lint_example_apps.sh --app librarian
```

### Exit Codes

- `0` - All apps passed (or only info-level findings)
- `1` - Warnings found (in strict mode)
- `2` - Errors found
- `3` - Build or execution error

### GitHub Actions Integration

> **Note:** DashFlow uses internal Dropbox CI, not GitHub Actions. The `.github/` directory does not exist in this repository. The workflow below is provided as a template for teams using GitHub Actions.

```yaml
jobs:
  lint-examples:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Build DashFlow CLI
        run: cargo build -p dashflow-cli --release

      - name: Lint example apps
        run: ./scripts/lint_example_apps.sh --sarif > lint-results.sarif

      - name: Upload SARIF results
        uses: github/codeql-action/upload-sarif@v2
        with:
          sarif_file: lint-results.sarif
```

### GitLab CI Integration

```yaml
lint_apps:
  stage: test
  script:
    - cargo build -p dashflow-cli --release
    - ./scripts/lint_example_apps.sh --json > lint-results.json
  artifacts:
    reports:
      codequality: lint-results.json
```

## Feedback Dashboard

For reviewing collected feedback from AI workers and users, DashFlow provides CLI commands to summarize and analyze feedback:

```bash
# View feedback summary with statistics
dashflow lint feedback summary

# List all unreviewed feedback
dashflow lint feedback list --unreviewed

# Export for external analysis (spreadsheet, dashboard)
dashflow lint feedback export -o feedback-export.json

# View feedback by category
dashflow lint feedback list --category observability

# View feedback by pattern
dashflow lint feedback list --pattern cost_tracking
```

### Feedback JSON Schema

Exported feedback follows this schema for integration with external dashboards:

```json
{
  "entries": [/* FeedbackEntry[] */],
  "stats_by_pattern": {
    "pattern_name": {
      "count": 5,
      "common_reasons": [{"reason": "...", "count": 3}],
      "enhancement_suggestions": 2
    }
  },
  "stats_by_category": {
    "category_name": {
      "count": 10,
      "patterns": ["pattern1", "pattern2"]
    }
  }
}
```

## IDE Integration Details

### VS Code with SARIF Viewer

1. Install the [SARIF Viewer](https://marketplace.visualstudio.com/items?itemName=MS-SarifVSCode.sarif-viewer) extension
2. Run: `dashflow lint --format sarif . > .vscode/lint.sarif`
3. Open `.vscode/lint.sarif` in VS Code
4. Warnings appear inline with code locations

### VS Code Tasks

Add to `.vscode/tasks.json`:

```json
{
  "version": "2.0.0",
  "tasks": [
    {
      "label": "DashFlow Lint",
      "type": "shell",
      "command": "dashflow lint --format sarif . > .vscode/lint.sarif",
      "problemMatcher": [],
      "group": "build"
    }
  ]
}
```

### IntelliJ IDEA with Qodana

The SARIF output is compatible with IntelliJ's Qodana analysis:

```bash
dashflow lint --format sarif . > lint-results.sarif
```

Import the SARIF file in IntelliJ via **Code > Analyze Code > Open SARIF Report**.

### Rust-Analyzer Integration (Future)

Custom rust-analyzer diagnostics integration is planned. In the meantime, use the SARIF viewer approach or the pre-commit hook for immediate feedback.

## See Also

- [Introspection Guide](./INTROSPECTION.md) - How introspection discovers platform capabilities
- [Best Practices](./BEST_PRACTICES.md) - General DashFlow development practices
- [CLI Reference](./CLI_REFERENCE.md) - Complete CLI documentation
