# DashFlow Testing Guide

**Last Updated:** 2026-01-02 (Worker #2337 - Add missing Last Updated headers)

This document describes DashFlow's testing strategy, infrastructure, and best practices.

## Table of Contents

- [Testing Philosophy](#testing-philosophy)
- [Canonical Test Scripts](#canonical-test-scripts)
- [Test Types](#test-types)
- [Running Tests](#running-tests)
- [Test Infrastructure](#test-infrastructure)
- [Writing Tests](#writing-tests)
- [Environment Variables](#environment-variables)
- [CI/CD Integration](#cicd-integration)
- [Debugging Tests](#debugging-tests)
- [Manual Observability Verification](#manual-observability-verification)

## Testing Philosophy

DashFlow follows these testing principles:

1. **Correctness First**: Tests verify behavior matches the Python baseline exactly
2. **Isolation**: Tests should not depend on external services unless explicitly designed as integration tests
3. **Speed**: Unit tests must be fast; slow tests are marked `#[ignore]`
4. **Determinism**: Tests should produce consistent results (no flaky tests)
5. **Coverage**: Every public API should have tests

## Canonical Test Scripts

DashFlow provides several test scripts for different purposes. Choose the right script based on what you're testing:

| Script | Purpose | Prerequisites | Duration |
|--------|---------|---------------|----------|
| `validate_tests.sh` | Pre-commit validation | None | ~3-5 min |
| `smoke_test_all_features.sh` | Core feature verification | `OPENAI_API_KEY` | ~5-10 min |
| `run_integration_tests.sh` | Docker-based integration | Docker running | ~10-15 min |
| `run_complete_eval.sh` | Full E2E with quality eval | API key + Kafka | ~15-20 min |

### Quick Decision Guide

**"Did I break anything?"** → `./scripts/validate_tests.sh`

Runs cargo check, unit tests, clippy, and formatting. No external services needed.

**"Do the main features work?"** → `./scripts/smoke_test_all_features.sh`

Tests 8 core features (bind_tools, create_react_agent, structured outputs, HITL, streaming, stream_events, add_messages, app compilation). Requires `OPENAI_API_KEY`.

**"Does everything work with real infrastructure?"** → `./scripts/run_integration_tests.sh`

Starts docker-compose test services, runs all integration tests (including `#[ignore]`), then stops services. Use `--keep` to leave services running.

```bash
./scripts/run_integration_tests.sh              # Run all, cleanup after
./scripts/run_integration_tests.sh --keep       # Keep services running
./scripts/run_integration_tests.sh --filter X   # Only tests matching X
```

**"What's the quality of the system output?"** → `./scripts/run_complete_eval.sh`

Runs comprehensive evaluation loop with Kafka and OpenAI. Generates quality reports in `eval_outputs/`.

### Other Useful Scripts

| Script | Purpose |
|--------|---------|
| `verify_and_checkpoint.sh` | Build verification with commit tracking (for AI workers) |
| `preflight.sh` | Pre-work environment check |
| `doctor.sh` | Repo health diagnostics (git, cargo, artifacts) |
| `check_test_infrastructure.sh` | Verify test utilities are working |

## Test Types

### Unit Tests

In-module tests that verify individual functions and types.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_my_function() {
        let result = my_function(42);
        assert_eq!(result, expected_value);
    }
}
```

**Location**: Same file as the code being tested, in a `mod tests` block.

**Run with**: `cargo test --lib`

### Integration Tests

Tests that verify multiple components work together correctly.

**Location**: `crates/<crate>/tests/*.rs`

**Run with**: `cargo test --test <test_name>` or `cargo test -p <crate>`

### End-to-End (E2E) Tests

Tests that exercise the entire system with real external services.

**Location**: `crates/dashflow-standard-tests/tests/`

**Run with**: `cargo test -p dashflow-standard-tests -- --ignored`

These tests require API keys and are marked `#[ignore]` by default.

### Load Tests

Performance and stress tests.

**Location**: `crates/dashflow-wasm-executor/tests/load_tests.rs` and others

**Run with**: `./scripts/run-load-test.sh` or `./scripts/load_test_apps.sh`

## Running Tests

### Quick Commands

```bash
# Run all unit tests
cargo test --lib

# Run all tests (excluding ignored)
cargo test --workspace

# Run tests for a specific crate
cargo test -p dashflow-openai

# Run a specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture

# Run ignored tests (requires API keys)
cargo test -- --ignored

# Run all tests including ignored
cargo test -- --include-ignored
```

### Using Nextest (Recommended)

DashFlow uses [cargo-nextest](https://nexte.st/) for better test execution:

```bash
# Install nextest
cargo install cargo-nextest

# Run all tests with nextest
cargo nextest run

# Run with specific profile
cargo nextest run --profile ci

# Run and show output
cargo nextest run --no-capture
```

Nextest configuration in `nextest.toml`:

| Profile | Timeout | Fail-Fast | Use Case |
|---------|---------|-----------|----------|
| `default` | 5 min | No | Local development |
| `ci` | 2 min | Yes | CI pipelines |

Test category overrides:

| Filter | Timeout | Description |
|--------|---------|-------------|
| `test(/load_test/)` | 4 min | Load tests |
| `test(/e2e/)` | 6 min | E2E tests |
| `test(/integration/)` | 4 min | Integration tests |

### Validation Scripts

See [Canonical Test Scripts](#canonical-test-scripts) for full details on which script to use.

```bash
./scripts/validate_tests.sh             # Pre-commit: compile + test + clippy + format
./scripts/smoke_test_all_features.sh    # Core features: requires OPENAI_API_KEY
./scripts/run_integration_tests.sh      # Docker infrastructure tests
./scripts/check_test_infrastructure.sh  # Verify test utilities
```

**Note**: `validate_tests.sh` enforces M-294 (no `unwrap()`/`expect()` in production targets). If intentional, use `#[allow(clippy::unwrap_used|expect_used)]` with a SAFETY justification comment.

## Test Infrastructure

### test-utils Crate

The `test-utils` crate provides shared testing infrastructure:

```rust
use test_utils::{
    // Credential management
    Credentials, CredentialsLoader,
    openai_credentials, anthropic_credentials,

    // Environment setup
    init_test_env, skip_slow_tests, skip_paid_tests,

    // Service health
    HealthChecker,

    // Docker helpers
    DockerServices,

    // Mock implementations
    MockEmbeddings,

    // Observability testing
    check_grafana_has_data, query_prometheus,
};
```

#### Credential Loading

```rust
#[tokio::test]
#[ignore]  // Requires credentials
async fn test_with_openai() {
    // Load from environment
    let creds = openai_credentials().expect("OPENAI_API_KEY required");

    // Or use the loader for multiple credentials
    let creds = CredentialsLoader::new()
        .require("OPENAI_API_KEY")
        .optional("OPENAI_ORG_ID")
        .load()
        .expect("Missing credentials");
}
```

#### Health Checks

```rust
use test_utils::HealthChecker;

#[tokio::test]
async fn test_requires_redis() {
    let health = HealthChecker::new()
        .check_redis("redis://localhost:6379")
        .await;

    if !health.is_healthy() {
        eprintln!("Skipping: Redis not available");
        return;
    }

    // Run test...
}
```

#### Mock Embeddings

```rust
use test_utils::MockEmbeddings;

#[test]
fn test_with_mock_embeddings() {
    let embeddings = MockEmbeddings::new(384);  // 384-dim vectors
    let vector = embeddings.embed("test text").unwrap();
    assert_eq!(vector.len(), 384);
}
```

### Common Test Utilities

Located in `crates/dashflow-standard-tests/tests/common/mod.rs`:

```rust
use common::{load_test_env, get_openai_key, has_openai_key};

// Verify answer quality
let answer = "The capital of France is Paris";
assert!(verify_answer_quality(answer, &["paris", "france"]));

// Extract numbers from text
let nums = extract_numbers("The answer is 42");
assert_eq!(nums, vec![42.0]);
```

## Writing Tests

### Unit Test Pattern

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Group related tests
    mod parsing {
        use super::*;

        #[test]
        fn test_parse_valid_input() {
            let result = parse("valid input");
            assert!(result.is_ok());
        }

        #[test]
        fn test_parse_invalid_input() {
            let result = parse("invalid");
            assert!(result.is_err());
        }
    }

    // Test error conditions
    mod errors {
        use super::*;

        #[test]
        fn test_returns_error_on_empty_input() {
            let result = process("");
            assert!(matches!(result, Err(Error::InvalidInput(_))));
        }
    }
}
```

### Integration Test Pattern

```rust
// tests/integration_test.rs

use dashflow::prelude::*;

/// Setup function for integration tests
fn setup() -> TestContext {
    let _ = dotenvy::dotenv();
    // Setup code...
}

#[tokio::test]
async fn test_full_workflow() {
    let ctx = setup();

    // Step 1: Create graph
    let graph = create_test_graph();

    // Step 2: Execute
    let result = graph.invoke(initial_state).await;

    // Step 3: Verify
    assert!(result.is_ok());
    assert_eq!(result.unwrap().output, expected_output);
}
```

### Async Test Pattern

```rust
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn test_async_operation() {
    // Use timeout to prevent hanging
    let result = timeout(
        Duration::from_secs(30),
        async_operation()
    ).await;

    assert!(result.is_ok(), "Operation timed out");
    assert!(result.unwrap().is_ok());
}
```

### Test with Fixtures

```rust
#[test]
fn test_with_fixture() {
    // Load test fixture
    let input = include_str!("fixtures/test_input.json");
    let expected = include_str!("fixtures/expected_output.json");

    let result = process(input);

    assert_eq!(result, expected);
}
```

### Parameterized Tests

```rust
#[test]
fn test_multiple_cases() {
    let cases = vec![
        ("input1", "expected1"),
        ("input2", "expected2"),
        ("input3", "expected3"),
    ];

    for (input, expected) in cases {
        let result = transform(input);
        assert_eq!(result, expected, "Failed for input: {}", input);
    }
}
```

## Environment Variables

### Test Control Variables

| Variable | Values | Description |
|----------|--------|-------------|
| `SKIP_SLOW_TESTS` | `true`/`1` | Skip tests marked as slow |
| `SKIP_PAID_TESTS` | `true`/`1` | Skip tests that incur API costs |
| `TEST_TIMEOUT` | seconds | Override default test timeout |
| `RUST_LOG` | log level | Set logging verbosity |

### API Keys for Integration Tests

| Variable | Service |
|----------|---------|
| `OPENAI_API_KEY` | OpenAI GPT models |
| `ANTHROPIC_API_KEY` | Claude models |
| `COHERE_API_KEY` | Cohere models |
| `GROQ_API_KEY` | Groq inference |
| `FIREWORKS_API_KEY` | Fireworks AI |
| `MISTRAL_API_KEY` | Mistral AI |
| `XAI_API_KEY` | xAI Grok models |

### Infrastructure Credentials

| Variable | Service |
|----------|---------|
| `REDIS_URL` | Redis connection |
| `DATABASE_URL` | PostgreSQL connection |
| `MONGODB_URI` | MongoDB connection |
| `PINECONE_API_KEY` | Pinecone vector DB |
| `QDRANT_URL` | Qdrant vector DB |
| `WEAVIATE_URL` | Weaviate vector DB |

## CI/CD Integration

### Pre-Commit Validation

Run before every commit:

```bash
./scripts/validate_tests.sh
```

This runs:
1. Compile all tests (including ignored)
2. Run unit tests
3. Run clippy (zero warnings)
4. Check formatting

### CI Pipeline Stages

1. **Compile**: `cargo check --workspace`
2. **Lint**: `cargo clippy --workspace -- -D warnings`
3. **Format**: `cargo fmt --all -- --check`
4. **Unit Tests**: `cargo nextest run --profile ci --lib`
5. **Integration Tests**: `cargo nextest run --profile ci` (with secrets)
6. **E2E Tests**: `cargo nextest run --profile ci -- --ignored` (with secrets)

### GitHub Actions Example

> **Note:** DashFlow uses internal Dropbox CI, not GitHub Actions. The `.github/` directory does not exist in this repository. The workflow below is provided as a template for teams using GitHub Actions.

```yaml
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: taiki-e/install-action@nextest

      - name: Run tests
        run: cargo nextest run --profile ci
        env:
          OPENAI_API_KEY: ${{ secrets.OPENAI_API_KEY }}
```

## Debugging Tests

### Verbose Output

```bash
# Show println! output
cargo test -- --nocapture

# Show test names as they run
cargo test -- --test-threads=1

# Combine both
cargo test -- --nocapture --test-threads=1
```

### Debug a Specific Test

```bash
# Run single test with output
cargo test test_name -- --nocapture

# Run with RUST_LOG
RUST_LOG=debug cargo test test_name -- --nocapture

# Run in release mode (for timing-sensitive tests)
cargo test --release test_name
```

### Debugging Hanging Tests

```bash
# Use nextest timeouts
cargo nextest run --profile default

# Or manual timeout
timeout 300 cargo test test_name

# Check for deadlocks with threads=1
cargo test -- --test-threads=1
```

### Test Filtering

```bash
# Run tests matching pattern
cargo test add_messages

# Run tests in specific module
cargo test checkpoint::

# Run tests in specific crate
cargo test -p dashflow-openai

# Exclude tests matching pattern
cargo test -- --skip slow
```

## Best Practices

### Do

- Mark slow tests with `#[ignore]`
- Use `#[tokio::test]` for async tests
- Include setup/teardown in tests that need it
- Test error conditions, not just success paths
- Use descriptive test names: `test_parse_returns_error_on_empty_input`
- Add timeouts to async operations

### Don't

- Don't make tests depend on execution order
- Don't use `unwrap()` without good reason (prefer `expect()` with message)
- Don't leave commented-out test code
- Don't test private implementation details
- Don't write tests that pass when they should fail

### Test Naming Convention

```rust
// Good names
#[test]
fn test_parse_valid_json_returns_struct() { }
fn test_parse_invalid_json_returns_error() { }
fn test_empty_input_returns_default() { }

// Bad names
#[test]
fn test_parse() { }  // Too vague
fn test1() { }       // Non-descriptive
fn it_works() { }    // Doesn't explain what
```

## Manual Observability Verification

Follow these steps to manually verify the observability stack is working correctly.

### Prerequisites

Start the full observability stack:

```bash
docker-compose -f docker-compose.dashstream.yml up -d
```

Wait for all services to be healthy (~30 seconds).

### Step 1: Verify Prometheus Targets

1. Open http://localhost:9090/targets
2. Verify these targets are **UP** (green):
   - `dashstream-quality` (prometheus-exporter:9090)
   - `dashstream-infrastructure` (websocket-server:3002)

**If targets are DOWN**: Check Docker logs for the respective services.

### Step 2: Generate Test Data

Run a query to generate observability data:

```bash
cargo run -p librarian --features dashstream -- query "What is Rust?"
```

Wait 30 seconds for data to propagate through Kafka to Prometheus.

### Step 3: Verify Prometheus Metrics

Query these metrics at http://localhost:9090/graph:

```promql
# Quality score (should return value between 0.0 and 1.0)
dashstream_quality_monitor_quality_score

# Total queries processed (should be > 0)
dashstream_quality_monitor_queries_total

# Verify rate is non-zero
rate(dashstream_quality_monitor_queries_total[5m])
```

**Expected**: Each query returns at least one time series with non-zero values.

### Step 4: Verify Grafana Dashboard

1. Open http://localhost:3000 (admin/admin)
2. Navigate to: Dashboards → DashFlow Quality Agent - Production Monitoring
3. Verify these panels show data (not "No data"):
   - **Current Quality Score**: Value between 0.0 and 1.0
   - **Success Rate**: Percentage (e.g., 90%)
   - **Request Rate**: Non-zero graph

**If panels show "No data"**:
- Click panel title → Edit → Run query manually
- Check the datasource is set to "Prometheus"
- Verify the query works in Prometheus directly

### Step 5: Verify E2E Data Flow

Run the automated E2E test:

```bash
cargo test -p dashflow-test-utils -- strict_e2e_observability_test --ignored
```

**Expected**: All 5 checks pass (Kafka, Quality, Prometheus, API, Grafana).

### Step 6: Lint Dashboard

Verify the dashboard has no semantic issues:

```bash
python3 scripts/lint_grafana_dashboard.py grafana/dashboards/*.json
```

**Expected**: No errors (exit code 0).

### Troubleshooting

| Symptom | Likely Cause | Fix |
|---------|--------------|-----|
| Prometheus targets DOWN | Container not started | `docker-compose logs <service>` |
| No metrics after query | Kafka consumer not running | Check prometheus-exporter logs |
| Grafana "No data" | Datasource UID mismatch | Re-provision Grafana datasources |
| Quality score = NaN | Division by zero in query | Check if total_queries > 0 |

## See Also

- [QUICKSTART.md](../QUICKSTART.md) - Getting started
- [ERROR_TYPES.md](./ERROR_TYPES.md) - Error handling
- [CLI_REFERENCE.md](./CLI_REFERENCE.md) - CLI commands
- [test-utils README](../test-utils/README.md) - Test utilities crate
