# Testing Overhaul Plan

**Created:** 2025-12-16
**Triggered By:** Meta-introspection revealing 43+ issues that tests failed to catch

---

## Why Tests Failed to Catch 43+ Issues

### Root Cause Analysis

#### 1. Massive Test Ignorance (~100+ ignored tests)
```
#[ignore] // Requires Kafka running
#[ignore] // Requires OPENAI_API_KEY
#[ignore] // Requires Redis server
#[ignore] // Requires PostgreSQL
```

**Problem:** The vast majority of integration tests are `#[ignore]` by default. CI runs `cargo test` which skips them. "Green CI" means nothing.

**Evidence:**
- `test-utils/tests/observability_pipeline.rs` - 6 tests ignored
- `crates/dashflow-openai/tests/` - 60+ tests ignored
- `crates/dashflow-postgres-checkpointer/tests/` - 6 tests ignored
- `crates/dashflow-redis-checkpointer/src/lib.rs` - 5 tests ignored

#### 2. Unit Tests Don't Test Integration
Tests like `test_load_traces_from_directory` test the function in isolation with synthetic data, but don't test:
- Whether the CLI actually calls the function
- Whether the function works with real trace files
- Whether the analysis uses the loaded traces

#### 3. Weak Assertions
```rust
assert!(t1.is_empty());  // "No triggers" - passes even if broken
assert!(dasher.next_plan().unwrap().is_none());  // "No plan" - passes if nothing works
assert_eq!(loaded, 0);  // Empty dir test - doesn't prove non-empty works
```

**Problem:** Many tests assert "nothing happened" which passes whether the code works or not.

#### 4. No End-to-End Validation
No test verifies:
- `dashflow self-improve analyze` produces meaningful output
- Trace files are actually read and analyzed
- Token counts are populated
- State transitions are captured

#### 5. Mock-Heavy Testing Hides Real Issues
Package registry tests use `get_mock_packages()`. Optimizer tests fall back to Bootstrap. Tests pass but real functionality doesn't exist.

#### 6. No Contract Tests
No tests verify:
- Trace JSON schema matches what executor produces
- CLI output matches what health check expects
- Metrics names match Prometheus configuration

---

## Testing Strategy Overhaul

### Tier 1: Critical Path Tests (P0 - Run in CI Always)

These tests MUST pass on every commit. No `#[ignore]`.

#### T1.1: Self-Improve Reads Traces (Issue #6)
```rust
#[test]
fn test_self_improve_reads_real_traces() {
    // Create real trace files
    let traces_dir = setup_test_traces();

    // Run CLI command
    let output = Command::new("dashflow")
        .args(["self-improve", "analyze"])
        .env("DASHFLOW_TRACES_DIR", traces_dir)
        .output()
        .unwrap();

    // Verify traces were analyzed
    assert!(output.stdout.contains("Capability gaps found:"));
    assert!(!output.stdout.contains("execution_count: 0"));
}
```

#### T1.2: Trace Data Quality (Issues #1-5, #7)
```rust
#[test]
fn test_executor_populates_trace_fields() {
    let graph = create_test_graph();
    let result = graph.invoke(test_state()).await;

    let trace = get_latest_trace();

    // Assert all fields are populated
    assert!(trace.thread_id.is_some(), "thread_id must be set");
    for node in &trace.nodes_executed {
        assert!(node.started_at.is_some(), "Node {} missing started_at", node.node);
        // tokens_used may be 0 for non-LLM nodes, but state should be captured
        assert!(node.state_before.is_some() || node.state_after.is_some(),
            "Node {} missing state snapshots", node.node);
    }
}
```

#### T1.3: No Deprecated API Usage (Issue #19)
```rust
// In CI: RUSTFLAGS='-D warnings' cargo check --workspace
// This catches deprecated API usage as hard errors
```

#### T1.4: Health Check Environment (Issue #8)
```rust
#[test]
fn test_health_check_reads_env() {
    env::set_var("OPENAI_API_KEY", "test-key");
    let output = Command::new("dashflow")
        .args(["introspect", "health"])
        .output()
        .unwrap();

    // Should not say "No LLM credentials found"
    assert!(!output.stdout.contains("No LLM credentials found"));
}
```

### Tier 2: Integration Tests (P1 - Run with Infrastructure)

These tests run when infrastructure is available (Kafka, Postgres, etc.).

#### T2.1: Alert Series Exist (Issues #27-29)
```rust
#[test]
#[ignore] // Requires Prometheus running
fn test_alert_metrics_have_series() {
    // Push test metrics
    emit_test_metrics();

    // Query Prometheus
    let metrics = vec![
        "websocket_e2e_latency_ms_bucket",
        "dashstream_dlq_backpressure_drops_total",
        "dashstream_rate_limit_exceeded_total",
    ];

    for metric in metrics {
        let result = prometheus_query(metric);
        assert!(!result.is_empty(), "Metric {} has no series", metric);
    }
}
```

#### T2.2: Commands Work with Thread ID (Issue #14)
```rust
#[test]
#[ignore] // Requires Kafka
fn test_commands_with_thread_id() {
    // Run a graph to generate thread_id
    let trace = run_test_graph();
    let thread_id = trace.thread_id.expect("thread_id should be set");

    // Verify commands work
    let commands = vec![
        vec!["replay", "--thread-id", &thread_id],
        vec!["flamegraph", "--thread-id", &thread_id],
        vec!["inspect", "--thread-id", &thread_id],
    ];

    for args in commands {
        let output = Command::new("dashflow").args(&args).output().unwrap();
        assert!(output.status.success(), "Command {:?} failed", args);
    }
}
```

### Tier 3: Contract Tests (P1 - Validate Schemas)

#### T3.1: Trace Schema Validation
```rust
#[test]
fn test_trace_schema_matches_executor_output() {
    let trace = create_trace_from_execution();
    let schema = include_str!("../schemas/execution_trace.json");

    validate_json_schema(&trace, schema).expect("Trace doesn't match schema");
}
```

#### T3.2: Prometheus Metrics Match Documentation
```rust
#[test]
fn test_documented_metrics_exist() {
    let docs = parse_metrics_docs("monitoring/PROMETHEUS_METRICS.md");
    let code_metrics = extract_metrics_from_code();

    for doc_metric in &docs {
        assert!(code_metrics.contains(doc_metric),
            "Documented metric {} not found in code", doc_metric);
    }
}
```

### Tier 4: Negative Tests (P2 - Verify Errors)

#### T4.1: Stub Implementations Fail Gracefully
```rust
#[test]
fn test_stub_retrievers_return_errors() {
    let retriever = WeaviateHybridSearchRetriever::new();
    let result = retriever.retrieve("test query").await;

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("stub implementation"));
}
```

---

## Tests to Deprecate/Remove

### Weak Tests (Assert Nothing)
These tests pass whether the code works or not:

| Test | File | Reason |
|------|------|--------|
| `test_trigger_system_per_execution` | self_improvement/integration.rs:1509 | Only asserts triggers are empty |
| `test_dasher_integration` | self_improvement/integration.rs:1589 | Asserts `next_plan().is_none()` |
| `test_load_traces_from_directory_empty` | self_improvement/integration.rs:2426 | Tests empty case only |
| `test_load_traces_from_directory_nonexistent` | self_improvement/integration.rs:2446 | Tests error case only |

### Mock-Only Tests
Tests that only test mock implementations:

| Test | File | Reason |
|------|------|--------|
| Package registry tests | pkg.rs | Only test mock data |
| Optimizer fallback tests | optimize.rs | Don't test actual optimizers |

### Duplicate/Redundant Tests
Tests that duplicate coverage:

| Test | Reason |
|------|--------|
| Multiple ignored OpenAI tests | Same pattern, different prompts |

---

## New Required Tests

### Issue Coverage Matrix

| Issue # | Required Test | Current Coverage |
|---------|--------------|------------------|
| 1 | Token tracking populated | NONE |
| 2 | State transitions captured | NONE |
| 3 | Per-node timestamps set | NONE |
| 4 | Parallelism tracked | NONE |
| 5 | Events emitted | NONE |
| 6 | Self-improve reads traces | WEAK (unit only) |
| 7 | Thread ID generated | NONE |
| 8 | Health check reads env | NONE |
| 9 | Prometheus metrics emitted | NONE |
| 14 | Commands work with thread_id | NONE |
| 19 | No deprecated API | CI check only |
| 27-29 | Alert metrics exist | NONE |

### Implementation Priority

1. **Add CI checks** (1 commit)
   - `RUSTFLAGS='-D warnings'` in cargo test
   - Node deps check before UI tests

2. **Add Tier 1 critical tests** (3-5 commits)
   - Self-improve trace reading
   - Trace data quality
   - Health check env reading

3. **Add contract tests** (2-3 commits)
   - Trace schema validation
   - Metrics documentation sync

4. **Enable existing ignored tests in CI** (2-3 commits)
   - With OpenAI API key in secrets
   - With Docker services started

---

## CI Pipeline Changes

### Current (Broken)
```yaml
- cargo test  # Runs only non-ignored tests
# No infrastructure, no API keys, no validation
```

### Proposed
```yaml
jobs:
  unit-tests:
    - cargo test  # Fast unit tests

  critical-integration:
    - cargo test --test critical_path  # Never ignored

  api-integration:
    needs: unit-tests
    env:
      OPENAI_API_KEY: ${{ secrets.OPENAI_API_KEY }}
    - cargo test -- --ignored openai  # OpenAI tests

  infra-integration:
    needs: unit-tests
    services:
      kafka: ...
      postgres: ...
    - cargo test -- --ignored infra  # Infrastructure tests

  contract-validation:
    - cargo test --test contracts  # Schema/doc validation
```

---

## Success Criteria

1. **CI catches Issue #19** (deprecated API) as hard failure
2. **CI catches Issue #6** (self-improve not reading traces)
3. **CI catches trace data quality issues** (#1-5, #7)
4. **No test uses `#[ignore]` without `--ignored` group tag**
5. **All documented metrics have series data in test environment**
6. **All CLI commands work with generated thread_id**

---

## Estimated Work

| Task | Commits |
|------|---------|
| CI pipeline changes | 2 |
| Tier 1 critical tests | 5 |
| Contract tests | 3 |
| Enable ignored tests | 3 |
| Remove weak tests | 1 |
| **Total** | **14** |
