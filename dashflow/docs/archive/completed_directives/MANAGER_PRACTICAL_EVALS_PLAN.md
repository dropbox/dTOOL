# Practical Evals & Quality Plan - Start From Scratch

**Date:** November 16, 2025
**User Question:** "I don't have any evals or a demo app. How would you do Option 1?"
**Reality:** You HAVE 10 demo apps, but NO systematic evaluation framework

---

## What You Actually Have

### ✅ Demo Apps (10 apps)
1. **document_search** - Document search agent (has tests!)
2. **advanced_rag** - RAG with retrieval grading
3. **code_assistant** - Code generation agent
4. **checkpoint_demo** - Checkpoint/resume patterns
5. **research_team** - Multi-agent orchestration
6. **error_recovery** - Error handling demo
7. **streaming_aggregator** - Streaming patterns
8. **document_search_hybrid** - Hybrid search
9. **document_search_optimized** - Optimized version
10. **document_search_streaming** - Streaming search

### ✅ Existing Tests
- `examples/apps/document_search/tests/multi_turn_conversations.rs` - 16 test scenarios
- `examples/apps/document_search/tests/performance.rs` - Performance benchmarks
- Quality module with LLM-as-judge (N=1552 validation)

### ❌ What's Missing
- **No golden dataset** - No stored expected outputs
- **No regression testing** - Tests run, but don't compare to baseline
- **No systematic evaluation** - Each app tests itself, no central framework
- **No CI integration** - Tests exist but not automated
- **No quality tracking** - Scores computed but not stored/compared

---

## REVISED PLAN: Build Evals Bottom-Up

### Phase 1: Single App Eval (Start Simple) - 5-8 commits

**Pick ONE app:** document_search (already has 16 test scenarios)

**Build:**

1. **Capture Golden Outputs (2 commits)**
   ```bash
   # Run app 16 times, save outputs
   cargo run --bin document_search --query "..." > golden/scenario_1.txt
   ```

   Create: `examples/apps/document_search/golden_dataset/`
   - scenario_01_simple_query.json
   - scenario_02_multi_turn.json
   - ... (16 total)

   Format:
   ```json
   {
     "id": "scenario_01",
     "query": "What is tokio?",
     "expected_output": "Tokio is...",
     "quality_threshold": 0.90,
     "must_contain": ["async", "runtime"],
     "must_not_contain": ["error", "unsupported"]
   }
   ```

2. **Build Eval Runner for This App (3 commits)**
   ```rust
   // examples/apps/document_search/src/bin/eval_runner.rs

   // 1. Load golden dataset
   // 2. Run document_search for each scenario
   // 3. Compare output to expected (LLM-as-judge)
   // 4. Generate report (pass/fail + scores)
   ```

3. **Add to CI (1 commit)**
   ```yaml
   # .github/workflows/eval_document_search.yml

   name: Document Search Evals
   on: [pull_request]
   jobs:
     eval:
       runs-on: ubuntu-latest
       steps:
         - run: cargo test --package document_search
         - run: cargo run --bin eval_runner --package document_search
         - run: if [ $quality_score < 0.90 ]; then exit 1; fi
   ```

4. **Validate (1-2 commits)**
   - Run eval_runner
   - Verify all 16 scenarios pass
   - Generate first eval report

**Deliverable:** ONE app with complete eval framework

---

### Phase 2: Generalize to Framework (8-10 commits)

**Extract reusable components:**

1. **Create dashflow-evals Crate (2 commits)**
   ```
   crates/dashflow-evals/
   ├── Cargo.toml
   └── src/
       ├── lib.rs
       ├── golden_dataset.rs   // Load/save/manage datasets
       ├── eval_runner.rs      // Generic eval execution
       ├── comparator.rs       // Compare outputs (LLM-as-judge)
       └── report.rs           // Generate eval reports
   ```

2. **Move document_search eval to use framework (2 commits)**
   ```rust
   // examples/apps/document_search/src/bin/eval.rs

   use dashflow_evals::{GoldenDataset, EvalRunner, Comparator};

   let dataset = GoldenDataset::load("golden_dataset/")?;
   let runner = EvalRunner::new(run_document_search);
   let results = runner.evaluate(&dataset).await?;
   results.print_report();
   ```

3. **Add Eval to 3 More Apps (3-4 commits)**
   - advanced_rag: Add golden dataset + eval
   - research_team: Add golden dataset + eval
   - checkpoint_demo: Add golden dataset + eval

4. **CI for All Apps (2 commits)**
   - Workflow that runs evals on all apps
   - Combined quality report

**Deliverable:** Reusable eval framework + 4 apps with evals

---

### Phase 3: Advanced Features (10-15 commits)

**Add sophisticated capabilities:**

1. **Regression Detection**
   - Store baseline results
   - Compare current run to baseline
   - Alert on score drops > 5%

2. **Multi-Dimensional Scoring**
   - Correctness, Relevance, Completeness, Safety
   - Per-dimension thresholds
   - Weighted overall score

3. **Performance Tracking**
   - Latency benchmarks
   - Cost tracking (tokens/query)
   - Compare to historical data

4. **Adversarial Testing**
   - Prompt injection scenarios
   - Out-of-distribution queries
   - Edge cases

**Deliverable:** Production-grade eval system

---

## Practical Implementation: Phase 1 Details

### Step-by-Step for document_search App

**Commit 1: Create golden dataset structure**
```bash
mkdir -p examples/apps/document_search/golden_dataset
cd examples/apps/document_search
```

Create 16 JSON files (one per scenario):
```json
// golden_dataset/01_simple_tokio_query.json
{
  "id": "01_simple_tokio_query",
  "description": "Simple factual query about tokio",
  "query": "What is tokio?",
  "context": "First turn, no conversation history",
  "expected_output_contains": [
    "async",
    "runtime",
    "Rust"
  ],
  "expected_output_not_contains": [
    "error",
    "unknown",
    "I don't know"
  ],
  "quality_threshold": 0.90,
  "max_latency_ms": 5000,
  "expected_tool_calls": ["search_documents"]
}
```

**Commit 2: Build eval runner binary**
```rust
// examples/apps/document_search/src/bin/eval.rs

use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Deserialize)]
struct GoldenScenario {
    id: String,
    query: String,
    expected_output_contains: Vec<String>,
    quality_threshold: f64,
    // ... other fields
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Load all golden scenarios
    let scenarios: Vec<GoldenScenario> = load_golden_dataset()?;

    // 2. Run document_search for each
    let mut results = Vec::new();
    for scenario in scenarios {
        let output = run_document_search(&scenario.query).await?;

        // 3. Compare to expected
        let passed = validate_output(&output, &scenario);
        let quality = score_quality(&scenario.query, &output).await?;

        results.push(EvalResult {
            scenario_id: scenario.id,
            passed,
            quality_score: quality,
        });
    }

    // 4. Generate report
    print_report(&results);

    // 5. Exit with error if any failed
    if results.iter().any(|r| !r.passed) {
        std::process::exit(1);
    }

    Ok(())
}

async fn run_document_search(query: &str) -> Result<String> {
    // Call the actual document_search main logic
    // Return the agent's response
}

fn validate_output(output: &str, scenario: &GoldenScenario) -> bool {
    // Check expected_output_contains
    for expected in &scenario.expected_output_contains {
        if !output.contains(expected) {
            return false;
        }
    }
    // Check expected_output_not_contains
    for unexpected in &scenario.expected_output_not_contains {
        if output.contains(unexpected) {
            return false;
        }
    }
    true
}

async fn score_quality(query: &str, output: &str) -> Result<f64> {
    // Use OpenAI to judge quality
    let judge_prompt = format!(
        "Rate the quality of this response on 0-1 scale:\n\
         Query: {}\n\
         Response: {}\n\
         Score (0-1):",
        query, output
    );
    // ... call gpt-4o-mini, parse score
}
```

**Commit 3: Add 16 golden scenarios**
- Create JSON files for all 16 test cases
- Use existing test queries from multi_turn_conversations.rs
- Run app once to capture baseline outputs

**Commit 4: Run and validate**
```bash
cargo run --bin eval --package document_search

# Output:
# Scenario 01: PASS (quality: 0.95)
# Scenario 02: PASS (quality: 0.92)
# ...
# 16/16 passed (100%)
```

**Commit 5: Add to CI**
```yaml
# .github/workflows/document_search_evals.yml
name: Document Search Quality Evals
on: [pull_request, push]
jobs:
  eval:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - run: cargo build --package document_search
      - run: cargo run --bin eval --package document_search
        env:
          OPENAI_API_KEY: ${{ secrets.OPENAI_API_KEY }}
```

---

## Why This Approach Works

**Advantages:**
1. **Start small** - One app, 16 scenarios
2. **Use existing tests** - multi_turn_conversations.rs already has queries
3. **Immediate value** - CI blocks bad changes to document_search
4. **Foundation for scale** - Once working, replicate to other 9 apps
5. **No conflicts** - Generates JSON data that observability worker can display

**Timeline:**
- Phase 1: 1-2 days (5-8 commits) - ONE app with complete evals
- Phase 2: 2-3 days (8-10 commits) - Framework + 4 apps
- Phase 3: 3-4 days (10-15 commits) - Advanced features

**Total: 6-9 days for complete eval system**

---

## Alternative: Even Simpler Start

**If you want to see results in 1-2 hours:**

**Commit 1:** Add eval.rs to document_search
**Commit 2:** Hardcode 3 test scenarios (simple, medium, complex)
**Commit 3:** Run and print report

```bash
cargo run --bin eval --package document_search

# Output:
# ✅ Simple query: PASS (0.95)
# ✅ Medium query: PASS (0.92)
# ✅ Complex query: PASS (0.88)
# 3/3 passed
```

Then iterate from there.

---

**User: Want to start with the simple 3-scenario approach (1-2 hours) or full Phase 1 (1-2 days)?**
