# Evaluation Best Practices

**Guide for writing effective evaluations with dashflow-evals**

This guide provides best practices for creating high-quality evaluation suites that catch bugs, prevent regressions, and maintain application quality over time.

---

## Table of Contents

1. [Golden Dataset Design](#golden-dataset-design)
2. [Quality Thresholds](#quality-thresholds)
3. [Test Scenario Coverage](#test-scenario-coverage)
4. [Performance Criteria](#performance-criteria)
5. [Regression Detection](#regression-detection)
6. [CI/CD Integration](#cicd-integration)
7. [Continuous Improvement](#continuous-improvement)
8. [Common Pitfalls](#common-pitfalls)

---

## Golden Dataset Design

### Start Small, Grow Strategically

**✅ DO:**
- Begin with 10-15 core scenarios covering happy paths
- Add edge cases incrementally as they're discovered
- Maintain a balanced distribution across difficulty levels
- Version control your golden dataset

**❌ DON'T:**
- Create 100+ scenarios without understanding coverage gaps
- Duplicate similar scenarios without clear differentiation
- Include flaky or non-deterministic test cases
- Forget to document why each scenario exists

### Scenario Structure

Each scenario should have:

1. **Clear Purpose**: What specific behavior does this test?
2. **Realistic Input**: Representative of production queries
3. **Explicit Expectations**: What must/must_not be in the output?
4. **Appropriate Thresholds**: Quality scores aligned with difficulty
5. **Performance Bounds**: Reasonable latency and cost limits

**Example - Good Scenario:**
```json
{
  "id": "multi_turn_context_preservation",
  "description": "Verify the agent maintains context across multiple conversation turns",
  "difficulty": "Medium",
  "category": "MultiTurn",
  "input": {
    "query": "What was the population I asked about earlier?",
    "conversation_history": [
      {"role": "user", "content": "What is the population of Tokyo?"},
      {"role": "assistant", "content": "Tokyo has approximately 14 million people."}
    ],
    "should_use_tools": []
  },
  "expected": {
    "must_contain": ["Tokyo", "14 million", "population"],
    "must_not_contain": ["error", "I don't have", "context"],
    "required_facts": [
      "References Tokyo from conversation history",
      "Recalls the population figure of 14 million"
    ]
  },
  "evaluation": {
    "quality_threshold": 0.90,
    "relevance_threshold": 0.95,
    "max_latency_ms": 3000,
    "max_cost_usd": 0.01
  }
}
```

**Why this is good:**
- Tests specific feature (context preservation)
- Realistic multi-turn conversation
- Clear success criteria
- Appropriate thresholds for medium difficulty

---

## Quality Thresholds

### Dimension-Specific Guidelines

Different scenarios require different emphasis on quality dimensions:

| Scenario Type | Accuracy | Relevance | Completeness | Safety | Coherence | Conciseness |
|---------------|----------|-----------|--------------|--------|-----------|-------------|
| Factual Lookup | **0.95** | 0.90 | 0.85 | 0.95 | 0.85 | 0.80 |
| Multi-Turn | 0.90 | **0.95** | 0.85 | 0.95 | **0.90** | 0.85 |
| Tool-Heavy | 0.90 | 0.90 | **0.95** | 0.95 | 0.85 | 0.80 |
| Adversarial | 0.85 | 0.85 | 0.80 | **0.98** | 0.80 | 0.75 |

### Overall Quality Threshold Selection

**High Stakes (0.95+):**
- Production-critical features
- Customer-facing responses
- Legal/compliance-sensitive content
- Financial or medical information

**Standard Quality (0.90-0.94):**
- Most application scenarios
- General knowledge questions
- Multi-turn conversations
- Tool orchestration

**Acceptable (0.85-0.89):**
- Complex reasoning tasks
- Ambiguous queries
- Adversarial test cases
- Edge case handling

**Developmental (<0.85):**
- Known limitations being fixed
- Experimental features
- Intentionally failing scenarios (for regression testing)

### Calibration Process

1. **Baseline Run**: Execute scenarios without thresholds
2. **Review Outputs**: Manually inspect a sample (10-20%)
3. **Set Initial Thresholds**: Based on current performance - 0.05
4. **Iterate**: Adjust based on false positive/negative rate
5. **Document Decisions**: Why each threshold was chosen

**Example Calibration Log:**
```
Scenario: complex_multi_step_reasoning
- Initial quality: 0.87 (avg over 10 runs)
- Manual review: 8/10 outputs acceptable
- Initial threshold: 0.82 (0.87 - 0.05 buffer)
- After 50 runs: 92% pass rate
- Adjusted threshold: 0.85 (reduce false positives)
- Final: 0.85 with 95% pass rate
```

---

## Test Scenario Coverage

### Coverage Dimensions

Ensure scenarios cover:

1. **Functional Coverage** (What the app does)
   - All major features
   - All tool types
   - All response formats

2. **Input Diversity** (How users interact)
   - Simple queries
   - Complex multi-step requests
   - Ambiguous phrasing
   - Multi-turn conversations
   - Edge cases (empty, very long, special characters)

3. **Difficulty Progression** (Increasing complexity)
   - Simple: 20% of scenarios (basic facts, single tool)
   - Medium: 40% of scenarios (multi-turn, reasoning)
   - Complex: 30% of scenarios (multi-tool, orchestration)
   - Adversarial: 10% of scenarios (injection, edge cases)

4. **Category Balance** (Scenario types)
   ```
   Factual:       20% (straightforward lookups)
   Multi-Turn:    25% (conversation flow)
   Tool Use:      25% (tool orchestration)
   Reasoning:     15% (complex logic)
   Edge Cases:    10% (unusual inputs)
   Adversarial:    5% (security testing)
   ```

### Coverage Tracking

Use the test generation module to identify gaps:

```rust
use dashflow_evals::test_generation::{CoverageGoals, TestGenerator};

let coverage_goals = CoverageGoals {
    min_scenarios_per_category: 10,
    min_scenarios_per_difficulty: 15,
    target_tool_coverage: 0.90,
    target_code_coverage: 0.85,
};

let gaps = generator.analyze_coverage(&dataset, &coverage_goals)?;
for gap in gaps {
    println!("Coverage gap: {} - Need {} more scenarios", gap.dimension, gap.needed);
}
```

---

## Performance Criteria

### Latency Budgets

Set realistic latency expectations based on scenario complexity:

**Simple Scenarios** (single LLM call, no tools):
- Target: <2 seconds
- Threshold: <3 seconds

**Medium Scenarios** (1-2 tool calls):
- Target: <5 seconds
- Threshold: <8 seconds

**Complex Scenarios** (3+ tool calls, multi-step):
- Target: <10 seconds
- Threshold: <15 seconds

**Adversarial Scenarios** (may fail/retry):
- Target: <5 seconds
- Threshold: <30 seconds (allow for retries)

### Cost Management

**Per-Scenario Budgets:**
- Simple: $0.005 (primarily fast model)
- Medium: $0.01 (mix of fast/premium)
- Complex: $0.02 (may require premium model)
- Adversarial: $0.03 (retries, premium model)

**Total Suite Budget:**
For a 50-scenario suite:
- Expected: $0.50-0.75 per run
- Maximum: $1.00 per run
- Monthly (daily CI): $20-30/month

**Cost Optimization Strategies:**
1. Use fast models (GPT-4o-mini) for simple scenarios
2. Batch similar scenarios to leverage caching
3. Implement smart model routing (fast → premium on low confidence)
4. Monitor cost trends and adjust thresholds

---

## Regression Detection

### Baseline Management

**When to Update Baselines:**

✅ **DO update when:**
- Intentional quality improvements (new model, better prompts)
- Architectural changes expected to improve performance
- New features added that enhance existing scenarios
- Bug fixes that improve correctness

❌ **DON'T update when:**
- Random performance fluctuations
- Trying to "make CI pass"
- Quality drops unexpectedly
- Just because tests are failing

### Statistical Significance

Always use statistical significance testing to avoid false alarms:

```rust
let regression_config = RegressionConfig {
    quality_drop_threshold: 0.05,        // 5% quality drop
    scenario_drop_threshold: 0.10,       // 10% per-scenario drop
    latency_increase_threshold: 0.20,    // 20% latency regression
    require_statistical_significance: true,
    significance_level: 0.05,            // 95% confidence
};
```

**Why this matters:**
- LLM outputs have inherent variance (~2-5%)
- Small sample sizes can show random fluctuations
- p-value <0.05 means 95% confidence the change is real

### Handling Regressions

**If regression detected:**

1. **Investigate**: What changed? (Code, model, data)
2. **Reproduce**: Run locally to confirm
3. **Diagnose**: Use detailed reports to find root cause
4. **Fix or Decide**:
   - Fix the bug if unintentional
   - Update expectations if intentional
   - Document if acceptable trade-off
5. **Verify**: Re-run evals to confirm fix

**Never:**
- Disable failing tests without investigation
- Lower thresholds just to pass CI
- Merge without understanding the regression

---

## CI/CD Integration

> **Note:** DashFlow uses internal Dropbox CI, not GitHub Actions. The `.github/` directory does not exist in this repository. The examples below are provided as templates for teams using GitHub Actions.

### Workflow Design

**Recommended Pipeline:**

```yaml
# .github/workflows/quality_gate.yml
name: Quality Gate

on:
  pull_request:
    paths:
      - 'src/**'
      - 'prompts/**'
      - 'golden_dataset/**'
  push:
    branches: [main]

jobs:
  evaluate:
    runs-on: ubuntu-latest

    steps:
      # 1. Run full eval suite
      - name: Run Evaluations
        run: cargo run --bin eval -- --output-json results.json

      # 2. Check quality gates
      - name: Enforce Quality Gates
        run: |
          quality=$(jq '.summary.avg_quality' results.json)
          if (( $(echo "$quality < 0.90" | bc -l) )); then
            echo "❌ Quality below threshold: $quality < 0.90"
            exit 1
          fi

      # 3. Compare to baseline (on PRs)
      - name: Check for Regressions
        if: github.event_name == 'pull_request'
        run: cargo run --bin eval -- --compare-baseline main

      # 4. Update baseline (on main)
      - name: Update Baseline
        if: github.event_name == 'push' && github.ref == 'refs/heads/main'
        run: cargo run --bin eval -- --save-baseline main

      # 5. Post PR comment
      - name: Comment Results
        uses: actions/github-script@v6
        with:
          script: |
            const fs = require('fs');
            const report = fs.readFileSync('report.md', 'utf8');
            github.rest.issues.createComment({
              issue_number: context.issue.number,
              body: report
            });
```

### Quality Gates Configuration

**Strict (Production Applications):**
```rust
QualityGate {
    min_pass_rate: 0.98,           // 98% scenarios must pass
    min_quality: 0.92,             // High quality bar
    max_latency_increase: 0.10,    // 10% max slowdown
    max_cost_increase: 0.10,       // 10% max cost increase
    block_on_new_failures: true,   // Zero new failures
}
```

**Balanced (Most Applications):**
```rust
QualityGate {
    min_pass_rate: 0.95,           // 95% scenarios pass
    min_quality: 0.90,             // Standard quality
    max_latency_increase: 0.20,    // 20% slowdown acceptable
    max_cost_increase: 0.15,       // 15% cost increase acceptable
    block_on_new_failures: true,   // Zero new failures
}
```

**Lenient (Development/Experimental):**
```rust
QualityGate {
    min_pass_rate: 0.90,           // 90% scenarios pass
    min_quality: 0.85,             // Acceptable quality
    max_latency_increase: 0.30,    // 30% slowdown acceptable
    max_cost_increase: 0.25,       // 25% cost increase acceptable
    block_on_new_failures: false,  // Allow new failures
}
```

---

## Continuous Improvement

### Feedback Loops

**1. Production Failures → New Scenarios**

When production issues occur:
```rust
// Add failing case to golden dataset
let scenario = TestScenario {
    id: "prod_failure_2025_11_17",
    description: "User reported incorrect response for <situation>",
    difficulty: Difficulty::Medium,
    category: ScenarioCategory::EdgeCase,
    // ... rest of scenario
};
```

**2. Human Review → Calibration**

Periodically review LLM judge accuracy:
```rust
use dashflow_evals::continuous_learning::HumanFeedback;

// Record human judgment
feedback.add_judgment(
    scenario_id: "complex_reasoning_01",
    llm_score: 0.87,
    human_score: 0.92,  // Human thought it was better
);

// Update calibration
let calibrator = feedback.build_calibrator()?;
```

**3. Uncertainty → Focus Testing**

Use confidence estimates to find uncertain scenarios:
```rust
// Scenarios where LLM judge is uncertain
let uncertain_scenarios = report.results
    .iter()
    .filter(|r| r.confidence < 0.80)
    .collect();

// Add human review or more specific assertions
```

### Iteration Cadence

**Daily:**
- Run full eval suite in CI (on PRs and merges)
- Monitor pass rate and quality trends
- Review and triage any new failures

**Weekly:**
- Analyze aggregated results for patterns
- Review uncertainty scenarios (low confidence)
- Update thresholds based on calibration data

**Monthly:**
- Deep dive into coverage gaps
- Generate new scenarios for uncovered areas
- Review and prune redundant scenarios
- Update baselines for intentional changes

**Quarterly:**
- Comprehensive eval suite audit
- Calibrate LLM judge against human review sample
- Review quality standards and thresholds
- Plan coverage expansion for new features

---

## Common Pitfalls

### 1. Over-Specified Expectations

**❌ BAD:**
```json
{
  "expected": {
    "full_text": "The capital of France is Paris, which is located on the Seine River in the north-central part of the country. It has a population of approximately 2.2 million people."
  }
}
```

**Problem**: Too rigid, fails on minor wording changes

**✅ GOOD:**
```json
{
  "expected": {
    "must_contain": ["Paris", "capital", "France"],
    "required_facts": [
      "Paris is the capital of France",
      "Located in north-central France"
    ],
    "semantic_similarity_threshold": 0.90
  }
}
```

**Benefit**: Flexible on wording, strict on facts

---

### 2. Unrealistic Thresholds

**❌ BAD:**
```json
{
  "evaluation": {
    "quality_threshold": 0.99,  // Too high for any real scenario
    "max_latency_ms": 500       // Unrealistic for multi-step task
  }
}
```

**Problem**: Creates false negatives, blocks good changes

**✅ GOOD:**
```json
{
  "evaluation": {
    "quality_threshold": 0.90,  // High but achievable
    "max_latency_ms": 5000      // Realistic for complexity
  }
}
```

---

### 3. Insufficient Coverage

**❌ BAD:**
- 50 scenarios all in "Simple" category
- No adversarial testing
- No multi-turn conversations

**✅ GOOD:**
- Balanced across difficulty levels (20/40/30/10 distribution)
- 5-10% adversarial scenarios
- 25% multi-turn conversations
- Coverage of all major features

---

### 4. Flaky Tests

**Signs of Flaky Tests:**
- Pass/fail changes without code changes
- Highly sensitive to random variations
- Depends on external state (time, network)

**Solutions:**
```rust
// Use retry logic for non-deterministic scenarios
EvalConfig {
    retry_on_failure: true,
    max_retries: 3,
    // ...
}

// Or mark as acceptable range
"quality_threshold": 0.85,  // Accept 0.85-1.00 range
```

---

### 5. Ignoring Performance

**❌ BAD:**
```json
{
  "evaluation": {
    "quality_threshold": 0.95,
    // No latency or cost constraints
  }
}
```

**Problem**: Quality improvements might come at 10x cost increase

**✅ GOOD:**
```json
{
  "evaluation": {
    "quality_threshold": 0.95,
    "max_latency_ms": 10000,    // Performance budget
    "max_cost_usd": 0.05        // Cost budget
  }
}
```

---

### 6. Baseline Drift

**❌ BAD Practices:**
- Never updating baselines (stale)
- Updating on every random fluctuation (unstable)
- Updating to "make CI pass" (hiding regressions)

**✅ GOOD Practices:**
- Update baselines only for intentional changes
- Require statistical significance before updating
- Document why baseline was updated
- Review baseline changes in code review

---

### 7. Poor Error Messages

**❌ BAD:**
```
Scenario 'complex_01' failed
Quality: 0.84 (threshold: 0.90)
```

**✅ GOOD:**
```
❌ Scenario 'complex_multi_step_reasoning' FAILED

Quality: 0.84 (threshold: 0.90)
Gap: -0.06

Dimension Breakdown:
- Accuracy: 0.92 ✓ (threshold: 0.90)
- Relevance: 0.88 ✓ (threshold: 0.85)
- Completeness: 0.76 ✗ (threshold: 0.85) ← FAILED
  Issue: Response missing key information about step 3

Suggestions:
- Add explicit step-by-step reasoning
- Ensure all sub-questions are addressed
- Consider increasing completeness prompt emphasis

View detailed report: eval_reports/scenario_complex_01.html
```

---

## Quick Reference Checklist

Before finalizing your eval suite:

- [ ] All scenarios have clear descriptions
- [ ] Balanced coverage across difficulty levels
- [ ] All major features have test coverage
- [ ] 5-10% adversarial/edge case scenarios
- [ ] Thresholds calibrated to actual performance
- [ ] Performance budgets defined (latency, cost)
- [ ] Baselines stored and version controlled
- [ ] Statistical significance enabled for regression detection
- [ ] CI/CD integrated with quality gates
- [ ] Reports are actionable (specific issues + suggestions)
- [ ] Documentation explains why each scenario exists
- [ ] Flaky tests identified and fixed
- [ ] Regular review cadence established

---

## Additional Resources

- **Tutorial**: [EVALUATION_TUTORIAL.md](./EVALUATION_TUTORIAL.md) - Step-by-step guide to creating your first eval suite
- **Evaluation Guide**: [EVALUATION_GUIDE.md](./EVALUATION_GUIDE.md) - Comprehensive reference for all evaluation features
- **Developer Experience**: [DEVELOPER_EXPERIENCE.md](./DEVELOPER_EXPERIENCE.md) - Tools and workflows for daily evaluation work
- **API Documentation**: Run `cargo doc --package dashflow-evals --open` for detailed API docs

---

## Support

If you encounter issues or have questions:

1. Check the [Troubleshooting Guide](./EVALUATION_TROUBLESHOOTING.md)
2. Review example scenarios in `examples/apps/librarian/data/`
3. File an issue on GitHub with reproduction steps

---

**Last Updated:** 2026-01-04
**Version:** 1.11
**Framework Version:** dashflow-evals 1.11
