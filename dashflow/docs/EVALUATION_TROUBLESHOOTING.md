# Evaluation Troubleshooting Guide

**Common issues and solutions for dashflow-evals**

This guide helps you diagnose and fix issues when working with the dashflow-evals framework.

---

## Table of Contents

1. [Test Failures](#test-failures)
2. [Quality Score Issues](#quality-score-issues)
3. [Performance Problems](#performance-problems)
4. [Regression Detection](#regression-detection)
5. [CI/CD Integration](#cicd-integration)
6. [Report Generation](#report-generation)
7. [Dataset Management](#dataset-management)

---

## Test Failures

### Scenario: "All tests suddenly failing"

**Symptoms:**
- Previously passing scenarios now fail
- Quality scores dropped significantly
- Multiple scenarios affected

**Common Causes & Solutions:**

#### 1. API Key Issues

**Check:**
```bash
echo $OPENAI_API_KEY
# Should output: sk-proj-...
```

**Fix:**
```bash
# Load from .env file
source .env

# Or export directly
export OPENAI_API_KEY="sk-proj-..."

# Verify
cargo run --bin eval -- --check-config
```

#### 2. Model Changes

**Check:**
```rust
// In your eval config
let config = EvalConfig {
    model: "gpt-4o",  // Did this change?
    // ...
};
```

**Issue**: Different models produce different outputs

**Fix:**
- Update baselines if model intentionally changed
- Or revert to previous model
- Document model version in dataset metadata

#### 3. Baseline Mismatch

**Check:**
```bash
# What baseline are you comparing against?
ls -la baselines/
cat baselines/latest_metadata.json
```

**Fix:**
```bash
# Regenerate baseline on current branch
cargo run --bin eval -- --save-baseline current

# Or specify correct baseline
cargo run --bin eval -- --compare-baseline <baseline_name>
```

---

### Scenario: "Single scenario failing intermittently"

**Symptoms:**
- Same scenario passes/fails randomly
- Quality score varies by >0.10 between runs
- "Flaky test" behavior

**Diagnosis:**
```bash
# Run scenario 10 times
for i in {1..10}; do
  cargo run --bin eval -- --scenario-id <scenario_id> >> flaky_test.log
done

# Check variance
grep "Quality:" flaky_test.log | awk '{sum+=$2; sumsq+=$2*$2} END {print "Mean:", sum/NR, "StdDev:", sqrt(sumsq/NR - (sum/NR)^2)}'
```

**Solutions:**

#### High Variance (StdDev > 0.05):

**Option 1: Lower threshold**
```json
{
  "evaluation": {
    "quality_threshold": 0.82  // Was 0.90, but variance is 0.85-0.95
  }
}
```

**Option 2: Enable retries**
```rust
EvalConfig {
    retry_on_failure: true,
    max_retries: 3,  // Pass if any of 3 attempts passes
    // ...
}
```

**Option 3: Use ensemble scoring**
```rust
// Run scenario 3 times, take median score
let scores = run_multiple_times(scenario, 3)?;
let median_score = calculate_median(scores);
```

---

### Scenario: "Expected content not found"

**Error:**
```
❌ Scenario 'factual_01' FAILED
Validation: must_contain check failed
Missing: ["async runtime"]
Actual output: "Tokio is an asynchronous runtime for Rust..."
```

**Cause**: Case sensitivity or exact matching

**Fix:**
```json
{
  "expected": {
    "must_contain": [
      "async",           // Changed from "async runtime"
      "runtime",         // Split into separate terms
      "Rust"
    ]
  }
}
```

**Alternative - Semantic matching:**
```json
{
  "expected": {
    "required_facts": [
      "Tokio is an async runtime"  // More flexible
    ],
    "semantic_similarity_threshold": 0.90
  }
}
```

---

## Quality Score Issues

### Scenario: "Quality scores too low"

**Symptoms:**
- Most scenarios scoring 0.70-0.80
- Below expected thresholds
- Outputs look correct to humans

**Diagnosis Steps:**

#### 1. Review Actual LLM Judge Reasoning

```bash
# Generate report with full details
cargo run --bin eval -- --output-html report.html --verbose

# Open report.html and expand scenario details
# Look at "Reasoning" field for each dimension
```

**Common Issues Found:**

**Issue 1: Judge misunderstanding expectations**
```
Judge reasoning: "Response lacks specific numbers"
Actual output: "Population is approximately 14 million"
```

**Fix**: Make expectations more explicit
```json
{
  "expected": {
    "required_facts": [
      "Population is stated with specific number",
      "Number should be in millions"
    ]
  }
}
```

**Issue 2: Judge being too strict**
```
Judge reasoning: "Response could be more concise (score: 0.60)"
Actual: Perfectly reasonable length
```

**Fix**: Adjust dimension weights
```rust
// De-emphasize conciseness for this scenario
"conciseness_threshold": 0.60  // Lower bar
```

#### 2. Check Judge Model

```rust
let judge_config = JudgeConfig {
    model: "gpt-4o",  // Using right model?
    temperature: 0.0,  // Should be deterministic
    // ...
};
```

**Best Practices:**
- Use `gpt-4o` or `gpt-4-turbo` for judging (not mini)
- Temperature should be 0.0 for consistency
- Enable structured output (JSON mode)

#### 3. Calibrate Judge

If judge consistently too harsh/lenient:

```rust
use dashflow_evals::continuous_learning::HumanFeedback;

// Add human judgments
let mut feedback = HumanFeedback::new();
feedback.add_judgment("scenario_01", 0.75, 0.90)?;  // LLM too harsh
feedback.add_judgment("scenario_02", 0.88, 0.85)?;  // LLM too lenient
// ... add 20-50 samples

// Build calibrator
let calibrator = feedback.build_calibrator()?;

// Apply to future scores
let calibrated = calibrator.calibrate(raw_score);
```

---

### Scenario: "Dimension scores inconsistent"

**Example:**
```
Accuracy: 0.95
Relevance: 0.93
Completeness: 0.45  ← Unexpectedly low
Safety: 0.98
```

**Diagnosis:**

```bash
# Get detailed reasoning for that dimension
cargo run --bin eval -- --scenario-id <id> --explain-dimension completeness
```

**Common Causes:**

1. **Missing information judge expects:**
   - Judge thinks response should include more details
   - Fix: Add explicit `required_facts` or lower threshold

2. **Judge hallucinating requirements:**
   - Judge expects facts not in expected output
   - Fix: Provide explicit expected output or context

3. **Rubric mismatch:**
   - Default rubric doesn't fit your use case
   - Fix: Customize rubric for your domain

---

## Performance Problems

### Scenario: "Evaluations taking too long"

**Symptoms:**
- 50 scenarios taking >20 minutes
- CI timeouts
- Blocking development workflow

**Diagnosis:**

```bash
# Profile evaluation run
cargo run --bin eval -- --profile --output-json profile.json

# Check breakdown
jq '.performance_breakdown' profile.json
```

**Optimization Strategies:**

#### 1. Increase Concurrency

```rust
EvalConfig {
    parallel_execution: true,
    max_concurrency: 10,  // Increase from default 5
    // ...
}
```

**Limits:**
- OpenAI rate limits: ~3000 requests/min (Tier 2)
- Don't exceed 20 concurrent for most use cases

#### 2. Use Faster Models

```rust
// For simple scenarios, use mini model
let simple_scenarios = dataset.filter(Difficulty::Simple);

EvalConfig {
    model: "gpt-4o-mini",  // 10x faster, 10x cheaper
    // ...
}
```

#### 3. Reduce Judge Overhead

**Option A: Skip judge for simple validations**
```json
{
  "expected": {
    "must_contain": ["fact1", "fact2"],
    "skip_llm_judge": true  // Just check contains
  }
}
```

**Option B: Batch scoring**
```rust
// Score multiple scenarios in one LLM call
judge.score_batch(&scenarios)?;
```

#### 4. Cache Results

```rust
// Enable result caching
EvalConfig {
    cache_results: true,
    cache_ttl: Duration::from_hours(24),
    // ...
}
```

---

### Scenario: "High evaluation costs"

**Symptoms:**
- $5+ per 50-scenario run
- Burning through API budget
- Costs increasing over time

**Cost Breakdown:**

```bash
# Analyze cost by component
cargo run --bin eval -- --cost-breakdown --output-json costs.json

# Example output:
# {
#   "total_cost": 5.23,
#   "by_component": {
#     "app_execution": 3.12,    // Your app's LLM calls
#     "quality_judging": 1.89,  // Judge LLM calls
#     "embeddings": 0.22        // Semantic similarity
#   }
# }
```

**Optimization:**

#### 1. Reduce App Costs

```rust
// Use cheaper models for evaluation
let eval_model = ChatOpenAI::new().with_model("gpt-4o-mini");  // Instead of gpt-4o

// Reduce tokens
let eval_config = ModelConfig {
    max_tokens: 500,  // Limit response length
    // ...
};
```

#### 2. Reduce Judge Costs

```rust
// Use mini model for simple scenarios
JudgeConfig {
    model: "gpt-4o-mini",  // 60x cheaper than gpt-4o
    // Only use gpt-4o for complex/adversarial
}
```

#### 3. Sample Scenarios

For rapid iteration during development:

```bash
# Run subset of scenarios
cargo run --bin eval -- --sample 10  # Random 10 scenarios

# Or specific categories
cargo run --bin eval -- --difficulty Simple  # Just simple ones
```

**Cost Target:**
- Development (frequent runs): $0.10-0.25 per run
- CI (full suite): $0.50-1.00 per run
- Production (comprehensive): $1.00-2.00 per run

---

## Regression Detection

### Scenario: "False regression alarms"

**Symptoms:**
- Regression detected, but outputs look the same
- Small quality drops flagged as critical
- Blocking PRs unnecessarily

**Cause**: Threshold too aggressive or missing statistical test

**Fix:**

```rust
RegressionConfig {
    quality_drop_threshold: 0.05,  // Increase from 0.02
    require_statistical_significance: true,  // Must enable
    significance_level: 0.05,      // 95% confidence
}
```

**Validation:**

```bash
# Run baseline comparison 10 times with no code changes
for i in {1..10}; do
  cargo run --bin eval -- --compare-baseline main >> regression_test.log
done

# Should show 0-1 regressions out of 10 runs
# If more, thresholds too strict
```

---

### Scenario: "Real regressions not detected"

**Symptoms:**
- Quality clearly dropped, but no alarm
- CI passes despite obvious problems
- Regressions only caught in production

**Diagnosis:**

```bash
# Compare detailed reports
cargo run --bin eval -- --compare-baseline main --verbose

# Look for:
# - Per-dimension changes (may cancel out in overall)
# - Specific scenarios affected
# - Performance regressions
```

**Solutions:**

#### 1. Tighten Thresholds

```rust
RegressionConfig {
    quality_drop_threshold: 0.03,     // More sensitive
    scenario_drop_threshold: 0.05,    // Flag individual scenarios
    latency_increase_threshold: 0.10, // Flag perf regressions
}
```

#### 2. Add Per-Dimension Checks

```rust
RegressionConfig {
    check_dimensions_individually: true,
    dimension_drop_threshold: 0.05,  // Flag dimension changes
}
```

#### 3. Increase Scenario Coverage

Missing regressions often means missing scenarios:

```bash
# Generate scenarios for uncovered areas
cargo run --bin test-generator -- \
  --analyze-coverage \
  --fill-gaps \
  --output-dataset expanded_dataset.json
```

---

## CI/CD Integration

> **Note:** DashFlow uses internal Dropbox CI, not GitHub Actions. The troubleshooting guidance below is provided as a reference for teams using GitHub Actions with DashFlow-based projects.

### Scenario: "GitHub Actions workflow failing"

**Common Issues:**

#### 1. Missing API Key

**Error:**
```
Error: Environment variable OPENAI_API_KEY not set
```

**Fix:**
```yaml
# In .github/workflows/eval.yml
env:
  OPENAI_API_KEY: ${{ secrets.OPENAI_API_KEY }}
```

**Setup:**
```bash
# Add secret to GitHub repo
gh secret set OPENAI_API_KEY --body "sk-proj-..."
```

#### 2. Timeout

**Error:**
```
Error: The operation was canceled.
(Job timed out after 10 minutes)
```

**Fix:**
```yaml
jobs:
  evaluate:
    timeout-minutes: 30  # Increase from 10
```

#### 3. Baseline Not Found

**Error:**
```
Error: Baseline 'main' not found
```

**Fix:**
```yaml
# Fetch baseline from artifact storage
- name: Download Baseline
  uses: actions/download-artifact@v3
  with:
    name: eval-baseline-main
    path: baselines/

# Or generate if not exists
- name: Generate Baseline
  if: steps.download.outcome == 'failure'
  run: |
    cargo run --bin eval -- --save-baseline main
```

---

### Scenario: "PR comments not posting"

**Possible Causes:**

#### 1. Missing Permissions

**Fix:**
```yaml
permissions:
  pull-requests: write  # Required for commenting
  contents: read
```

#### 2. Wrong Token Scope

**Fix:**
```yaml
- name: Comment PR
  uses: actions/github-script@v6
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}  # Must use GITHUB_TOKEN
```

#### 3. Rate Limiting

If posting large comments, GitHub may rate limit.

**Fix**: Use summary instead of full report
```yaml
- name: Generate Summary
  run: |
    cargo run --bin eval -- --summary-only --output-markdown summary.md
```

---

## Report Generation

### Scenario: "HTML report not displaying correctly"

**Symptoms:**
- Charts not rendering
- Styles broken
- Missing data

**Fixes:**

#### 1. Check Output Path

```bash
# Ensure output directory exists
mkdir -p target/eval_reports

# Generate with explicit path
cargo run --bin eval -- --output-html target/eval_reports/report.html
```

#### 2. Check Browser Compatibility

**Issue**: Some browsers block local file access

**Fix**: Serve via HTTP
```bash
# Simple HTTP server
cd target/eval_reports
python3 -m http.server 8000

# Open http://localhost:8000/report.html
```

#### 3. Regenerate with Verbose Logging

```bash
cargo run --bin eval -- --output-html report.html --verbose 2>&1 | tee report_generation.log

# Check for errors in log
```

---

### Scenario: "Charts missing in report"

**Cause**: Chart data not generated

**Check:**
```bash
# Verify chart data exists
cargo run --bin eval -- --output-json report.json
jq '.charts' report.json  # Should have quality_histogram, latency_chart, etc.
```

**Fix**:
```rust
// Ensure chart generation enabled
ReportConfig {
    include_charts: true,
    chart_types: vec![
        ChartType::QualityHistogram,
        ChartType::LatencyOverTime,
        ChartType::CostBreakdown,
    ],
}
```

---

## Dataset Management

### Scenario: "Dataset fails to load"

**Error:**
```
Error: Failed to parse golden dataset
Caused by: missing field `evaluation` at line 42
```

**Diagnosis:**

```bash
# Validate dataset
cargo run --bin dataset-manager -- validate golden_dataset.json

# Check for common issues:
# - Missing required fields
# - Invalid JSON syntax
# - Duplicate IDs
```

**Fix:**

```bash
# Auto-fix common issues
cargo run --bin dataset-manager -- fix golden_dataset.json --output fixed_dataset.json

# Or manually edit problematic scenario
```

---

### Scenario: "Too many scenarios, hard to manage"

**Solution 1: Split by category**

```bash
# Split into multiple datasets
cargo run --bin dataset-manager -- split golden_dataset.json \
  --by-category \
  --output-dir golden_dataset_split/

# Results:
# - golden_dataset_split/factual.json
# - golden_dataset_split/multi_turn.json
# - golden_dataset_split/adversarial.json
```

**Solution 2: Use imports**

```json
{
  "metadata": { "name": "main_dataset" },
  "imports": [
    "datasets/factual_scenarios.json",
    "datasets/adversarial_scenarios.json"
  ],
  "scenarios": [
    // Only new scenarios here
  ]
}
```

---

## Getting Help

If these solutions don't resolve your issue:

1. **Check Examples**: Review `examples/apps/librarian/` for working integration
2. **Run Diagnostics**: `cargo run --bin eval -- --diagnose`
3. **Enable Verbose Logging**: `RUST_LOG=debug cargo run --bin eval -- ...`
4. **File Issue**: Include:
   - Full error message
   - Minimal reproduction case
   - Output of `cargo run --bin eval -- --version`
   - Environment (OS, Rust version, model used)

---

## Quick Diagnostic Commands

```bash
# Check configuration
cargo run --bin eval -- --check-config

# Validate dataset
cargo run --bin dataset-manager -- validate <dataset.json>

# Test single scenario
cargo run --bin eval -- --scenario-id <id> --verbose

# Profile performance
cargo run --bin eval -- --profile

# Compare to baseline
cargo run --bin eval -- --compare-baseline <name> --verbose

# Check coverage
cargo run --bin test-generator -- --analyze-coverage <dataset.json>

# Verify judge calibration
cargo run --bin eval -- --calibrate-judge --sample-size 20
```

---

**Last Updated:** 2026-01-04
**Version:** 1.11
**Framework Version:** dashflow-evals 1.11
