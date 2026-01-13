# DashFlow Evaluation Guide

**Last Updated:** 2026-01-02 (Worker #2337 - Add missing Last Updated headers)
**Version:** 1.11
**Date:** December 16, 2025
**Status:** Production-Ready

---

## Overview

**dashflow-evals** is a comprehensive evaluation framework for testing DashFlow applications with:
- Automated quality scoring (6 dimensions)
- Regression detection
- Golden datasets (expected outputs)
- CI/CD integration
- Beautiful reports

**Think of it as pytest + coverage + regression testing, but for LLM applications.**

---

## Quick Start (5 Minutes)

### 1. Set API Key

```bash
export OPENAI_API_KEY="sk-..."
```

### 2. Run Evaluation

```bash
# Evaluate librarian app (50 scenarios)
cargo run -p librarian -- eval

# Output:
# Running evaluation on 50 scenarios...
# [====================] 50/50 (100%)
#
# === Evaluation Report ===
# Pass Rate: 48/50 (96%)
# Avg Quality: 0.924
# Avg Latency: 2.3s
# Total Cost: $0.42
```

### 3. View Report

```bash
# HTML report (interactive)
open target/eval_reports/eval_report_2025-11-16.html

# JSON (for CI)
cat target/eval_reports/eval_results.json
```

---

## What Gets Evaluated

### Quality Dimensions (6 total)

**1. Accuracy (0-1)**
- Factual correctness
- No hallucinations
- Matches expected facts

**2. Relevance (0-1)**
- Answers the actual question
- Not off-topic
- Addresses user intent

**3. Completeness (0-1)**
- Covers all aspects
- Nothing important missing
- Thorough explanation

**4. Safety (0-1)**
- No harmful content
- No biased language
- No PII leakage

**5. Coherence (0-1)**
- Logical flow
- Well-structured
- Easy to follow

**6. Conciseness (0-1)**
- Not unnecessarily verbose
- Clear and direct
- Efficient communication

**Overall Score:** Weighted average of all dimensions

---

## Golden Dataset Format

### Test Scenario Structure

```json
{
  "id": "01_simple_tokio_query",
  "description": "Basic factual query about tokio",
  "difficulty": "Simple",
  "category": "Factual",

  "input": {
    "query": "What is tokio?",
    "conversation_history": [],
    "context": "User needs basic explanation",
    "should_use_tools": ["search_documents"]
  },

  "expected": {
    "must_contain": ["async", "runtime", "Rust"],
    "must_not_contain": ["error", "unknown"],
    "required_facts": [
      "Tokio is an async runtime",
      "Used for Rust programming"
    ],
    "semantic_similarity_threshold": 0.85
  },

  "evaluation": {
    "quality_threshold": 0.90,
    "accuracy_threshold": 0.95,
    "relevance_threshold": 0.90,
    "completeness_threshold": 0.85,
    "max_latency_ms": 5000,
    "max_tokens": 500,
    "max_cost_usd": 0.01
  }
}
```

### Difficulty Levels

- **Simple:** Basic queries, single-turn, factual
- **Medium:** Multi-turn, requires reasoning
- **Complex:** Multi-step, multiple tools
- **Adversarial:** Edge cases, security testing

### Categories

- **Factual:** Direct information requests
- **MultiTurn:** Conversations with context
- **ToolUse:** Requires calling tools
- **Reasoning:** Multi-step logic
- **EdgeCase:** Unusual inputs
- **Adversarial:** Security/robustness testing

---

## CLI Usage

### Basic Evaluation

```bash
# Run all scenarios
cargo run -p librarian -- eval

# Run specific difficulty
cargo run -p librarian -- eval --difficulty Simple

# Run specific category
cargo run -p librarian -- eval --category Factual

# Limit number of scenarios
cargo run -p librarian -- eval --limit 10
```

### Baseline Management

```bash
# Save results as baseline
cargo run -p librarian -- eval --save-baseline main

# Compare to baseline
cargo run -p librarian -- eval --compare-baseline main

# List baselines
cargo run -p librarian -- eval --list-baselines
```

### Output Formats

```bash
# Generate HTML report (default)
cargo run --bin eval -- --output-html eval_report.html

# Generate JSON (for CI)
cargo run --bin eval -- --output-json results.json

# Generate Markdown (for GitHub)
cargo run --bin eval -- --output-markdown report.md

# All formats
cargo run --bin eval -- --output-all
```

---

## Adding Evals to Your App

### Step 1: Create Golden Dataset

```bash
# Create directory
mkdir -p examples/apps/my_app/golden_dataset

# Create scenarios
cat > examples/apps/my_app/golden_dataset/01_basic_test.json <<EOF
{
  "id": "01_basic_test",
  "description": "Basic functionality test",
  "difficulty": "Simple",
  "category": "Factual",
  "input": {
    "query": "Test query",
    "conversation_history": [],
    "should_use_tools": []
  },
  "expected": {
    "must_contain": ["expected", "output"],
    "must_not_contain": ["error"],
    "required_facts": ["Key fact 1"]
  },
  "evaluation": {
    "quality_threshold": 0.90,
    "max_latency_ms": 5000
  }
}
EOF
```

### Step 2: Create Eval Binary

```rust
// examples/apps/my_app/src/bin/eval.rs

use dashflow_evals::{
    eval_runner::EvalRunner,
    golden_dataset::GoldenDataset,
    quality_judge::MultiDimensionalJudge,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Load dataset
    let dataset = GoldenDataset::load_from_dir("golden_dataset/")?;

    // Create judge
    let judge = MultiDimensionalJudge::new("gpt-4o-mini")?;

    // Create runner
    let runner = EvalRunner::new(judge);

    // Define how to run your app
    let app_runner = |scenario: &TestScenario| {
        Box::pin(async move {
            // Your app logic here
            let result = my_app_invoke(&scenario.input.query).await?;
            Ok(result.to_string())
        })
    };

    // Run evaluation
    let results = runner.evaluate(&dataset, app_runner).await?;

    // Generate report
    results.print_report();
    results.save_html("eval_report.html")?;

    // Exit code based on pass/fail
    if results.pass_rate() < 0.95 {
        std::process::exit(1);
    }

    Ok(())
}
```

### Step 3: Run It

```bash
cargo run --bin eval --package my_app
```

---

## Regression Detection

### How It Works

1. **Baseline:** Save results from main branch
   ```bash
   git checkout main
   cargo run --bin eval -- --save-baseline main
   ```

2. **Feature Branch:** Compare to baseline
   ```bash
   git checkout my-feature
   cargo run --bin eval -- --compare-baseline main
   ```

3. **Report:** See regressions
   ```
   === Regression Report ===

   ⚠️ Quality Regression Detected
   - Overall quality: 0.924 → 0.854 (-7.6%)
   - Threshold: 5% (EXCEEDED)

   Failed Scenarios (5):
   - Scenario 12: 0.95 → 0.72 (-24%)
   - Scenario 23: 0.89 → 0.81 (-9%)
   ...
   ```

### Thresholds

Configure in evaluation config:

```rust
RegressionConfig {
    quality_drop_threshold: 0.05,      // Fail if quality drops >5%
    scenario_drop_threshold: 0.10,     // Fail if any scenario drops >10%
    latency_increase_threshold: 0.20,  // Fail if latency up >20%
    require_statistical_significance: true,  // Use t-test
    significance_level: 0.05,          // 95% confidence
}
```

---

## CI/CD Integration

> **Note:** DashFlow uses internal Dropbox CI, not GitHub Actions. The `.github/` directory does not exist in this repository. The examples below are provided as templates for teams using GitHub Actions.

### GitHub Actions

```yaml
# .github/workflows/my_app_evals.yml

name: My App Quality Evaluation
on: [pull_request, push]

jobs:
  evaluate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Run evaluation
        run: cargo run --bin eval --package my_app -- --output-json results.json
        env:
          OPENAI_API_KEY: ${{ secrets.OPENAI_API_KEY }}

      - name: Check quality gate
        run: |
          quality=$(jq '.summary.avg_quality' results.json)
          if (( $(echo "$quality < 0.90" | bc -l) )); then
            echo "❌ Quality below threshold"
            exit 1
          fi

      - name: Comment on PR
        uses: actions/github-script@v6
        with:
          script: |
            const fs = require('fs');
            const report = fs.readFileSync('eval_report.md', 'utf8');
            github.rest.issues.createComment({
              issue_number: context.issue.number,
              body: report
            });
```

### Quality Gates

Block PRs if:
- ✅ Pass rate < 95%
- ✅ Quality score < 0.90
- ✅ Quality drops >5% vs baseline
- ✅ Latency increases >20%
- ✅ New failures introduced

---

## Reports

### HTML Report (Interactive)

**Features:**
- Summary cards (pass rate, quality, cost)
- Interactive charts (quality distribution, latency)
- Per-scenario breakdown (expandable)
- Diff viewer (expected vs actual)
- Filtering by difficulty/category
- Sorting by quality/latency

**Location:** `target/eval_reports/eval_report.html`

### JSON Report (CI/Dashboards)

**Structure:**
```json
{
  "summary": {
    "total_scenarios": 50,
    "passed": 48,
    "failed": 2,
    "pass_rate": 0.96,
    "avg_quality": 0.924,
    "avg_latency_ms": 2300,
    "total_cost_usd": 0.42
  },
  "scenarios": [...],
  "regressions": [...],
  "metadata": {...}
}
```

### Markdown Report (GitHub)

Formatted for PR comments:
```markdown
## 📊 Evaluation Report

**Pass Rate:** 48/50 (96%)
**Avg Quality:** 0.924
**Avg Latency:** 2.3s

### Results by Category
| Category | Pass Rate | Avg Quality |
|----------|-----------|-------------|
| Factual | 100% | 0.95 |
| Multi-Turn | 93% | 0.92 |
...
```

---

## Advanced Features

### Multi-Model Comparison

```rust
// Compare GPT-4o-mini vs GPT-4
let models = vec!["gpt-4o-mini", "gpt-4"];
let comparison = runner.compare_models(&dataset, models).await?;

// Report shows which model is better for which scenarios
```

### Adversarial Testing

> **Note:** The `generate-adversarial` binary is a planned feature. Currently, create adversarial scenarios manually.

```bash
# Planned: Generate adversarial scenarios
# cargo run --bin generate-adversarial --package dashflow-evals

# Test against adversarial scenarios (once created)
cargo run --bin eval -- --category Adversarial
```

### Performance Tracking

```rust
// Track quality, latency, cost over time
let analyzer = TrendAnalyzer::new("baselines/")?;
let trends = analyzer.analyze()?;

println!("Quality trend (last 30 commits): {:?}", trends.quality_trend);
// Output: [0.89, 0.91, 0.92, 0.94] ← improving!
```

---

## Best Practices

### 1. Start with 10-20 Scenarios

Don't create 500 scenarios on day 1. Start small:
- 5 simple queries
- 5 medium complexity
- 5 edge cases

Expand as you find gaps.

### 2. Balance Coverage

Aim for:
- 40% Simple (basics work)
- 30% Medium (handles complexity)
- 20% Complex (advanced features)
- 10% Adversarial (robustness)

### 3. Capture Real User Queries

Best test scenarios come from production:
```rust
// Extract from logs
let scenarios = ScenarioGenerator::generate_from_logs(production_logs)?;
```

### 4. Review Failed Scenarios

When eval fails:
1. Check HTML report (see actual vs expected)
2. Understand why it failed
3. Fix app OR update expected output
4. Re-run

### 5. Update Baselines Regularly

After significant improvements:
```bash
cargo run --bin eval -- --save-baseline v1.2.0
```

### 6. Monitor Trends

Weekly/monthly, use the trends module API:
```rust
use dashflow_evals::trends::TrendAnalyzer;

let analyzer = TrendAnalyzer::new("baselines/")?;
let trends = analyzer.analyze()?;
println!("Trend: {:?}", trends);
```

---

## Integration with Observability

**Evals → Kafka → Dashboards**

```rust
// Evaluation results publish to Kafka
let producer = DashStreamProducer::new("kafka:9092", "dashstream-evals")?;

// After evaluation
producer.emit_eval_results(&results).await?;

// Observability dashboard shows:
// - Quality trends
// - Pass/fail rates
// - Per-scenario performance
```

**Clean separation:**
- dashflow-evals: Generates data (backend)
- observability-ui: Visualizes data (frontend)

---

## Troubleshooting

### "OPENAI_API_KEY not set"

```bash
export OPENAI_API_KEY="sk-..."
```

### "Scenario timeout"

Increase timeout in scenario JSON:
```json
{
  "evaluation": {
    "max_latency_ms": 10000  // Increase from 5000
  }
}
```

### "All scenarios failing"

Check your app still works:
```bash
cargo run -p librarian -- query "test"
```

### "Quality scores seem wrong"

Check judge model:
- Using gpt-4o-mini by default
- Can switch to gpt-4 for better accuracy
- Calibrate against human judgments

---

## API Reference

### GoldenDataset

```rust
use dashflow_evals::golden_dataset::GoldenDataset;

// Load
let dataset = GoldenDataset::load_yaml("dataset.yaml")?;
let dataset = GoldenDataset::load_json("dataset.json")?;
let dataset = GoldenDataset::load_from_dir("golden_dataset/")?;

// Filter
let simple = dataset.filter(Some(Difficulty::Simple), None);
let factual = dataset.filter(None, Some(ScenarioCategory::Factual));

// Validate
let warnings = dataset.validate()?;

// Save
dataset.save_yaml("dataset.yaml")?;
```

### EvalRunner

```rust
use dashflow_evals::eval_runner::EvalRunner;

let runner = EvalRunner::new(judge)
    .with_config(EvalConfig {
        parallel_execution: true,
        max_concurrency: 10,
        retry_on_failure: true,
        max_retries: 2,
        scenario_timeout: Duration::from_secs(30),
        ..Default::default()
    });

let results = runner.evaluate(&dataset, app_runner).await?;
```

### RegressionDetector

```rust
use dashflow_evals::regression::RegressionDetector;

let detector = RegressionDetector::new(RegressionConfig {
    quality_drop_threshold: 0.05,    // 5%
    require_statistical_significance: true,
    ..Default::default()
});

let regressions = detector.detect(&baseline, &current)?;

if !regressions.is_empty() {
    // Handle regressions
}
```

---

## Examples

### Example 1: Simple Evaluation

```rust
use dashflow_evals::{GoldenDataset, EvalRunner, MultiDimensionalJudge};

#[tokio::main]
async fn main() -> Result<()> {
    let dataset = GoldenDataset::load_from_dir("golden_dataset/")?;
    let judge = MultiDimensionalJudge::new("gpt-4o-mini")?;
    let runner = EvalRunner::new(judge);

    let results = runner.evaluate(&dataset, |scenario| {
        Box::pin(async move {
            // Run your app
            let output = my_app(&scenario.input.query).await?;
            Ok(output)
        })
    }).await?;

    results.print_report();
    Ok(())
}
```

### Example 2: Regression Detection

```rust
use dashflow_evals::{BaselineStore, RegressionDetector};

#[tokio::main]
async fn main() -> Result<()> {
    // Load baseline
    let store = BaselineStore::new("baselines/")?;
    let baseline = store.load_baseline("main")?;

    // Run current evaluation
    let current = run_evaluation().await?;

    // Detect regressions
    let detector = RegressionDetector::default();
    let regressions = detector.detect(&baseline, &current)?;

    if !regressions.is_empty() {
        println!("❌ Regressions detected:");
        for reg in regressions {
            println!("  - {}", reg.description);
        }
        std::process::exit(1);
    }

    println!("✅ No regressions");
    Ok(())
}
```

### Example 3: CI Integration

```rust
// In your CI script
use dashflow_evals::ci::QualityGate;

let gate = QualityGate {
    min_pass_rate: 0.95,
    min_quality: 0.90,
    max_latency_increase: 0.20,
    block_on_new_failures: true,
};

let results = run_evaluation().await?;
let gate_result = gate.check(&results, Some(&baseline))?;

if !gate_result.passed {
    println!("❌ Quality gate failed:");
    for reason in gate_result.reasons {
        println!("  - {}", reason);
    }
    std::process::exit(1);
}
```

---

## FAQ

### Q: How long does evaluation take?

**A:** Depends on scenarios and concurrency:
- 10 scenarios, sequential: ~1-2 minutes
- 50 scenarios, parallel (10 concurrent): ~3-5 minutes
- 100 scenarios, parallel: ~8-12 minutes

### Q: How much does it cost?

**A:** Uses GPT-4o-mini for judging:
- ~$0.008 per scenario evaluation
- 50 scenarios ≈ $0.40 per run
- Can use cached results to reduce cost

### Q: What if my app uses external services?

**A:** Mock them or use test instances:
```rust
// Option 1: Mock
let mock_chroma = MockVectorStore::new();

// Option 2: Test instance
let test_chroma = ChromaClient::new("http://test-chroma:8000");
```

### Q: How do I handle flaky tests?

**A:** Enable retry:
```rust
EvalConfig {
    retry_on_failure: true,
    max_retries: 2,  // Retry up to 2 times
    ..Default::default()
}
```

### Q: Can I run evals locally without CI?

**A:** Yes! Just run:
```bash
cargo run --bin eval --package your_app
```

---

## Next Steps

1. **Add evals to your app** - Follow "Adding Evals to Your App" guide
2. **Run first evaluation** - See how your app performs
3. **Set baseline** - Capture current quality as baseline
4. **Add to CI** - Automate testing on every PR
5. **Monitor trends** - Track quality over time

---

## Resources

- **Crate docs:** `cargo doc --package dashflow-evals --open`
- **Examples:** See `examples/apps/librarian/` for production evaluation patterns
- **Golden datasets:** `examples/apps/librarian/data/`
- **Reports:** `target/eval_reports/`

---

**Questions? Issues? File a bug or ask in #dashflow**
