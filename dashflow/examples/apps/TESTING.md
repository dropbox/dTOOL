# Example App Testing Guide

This guide explains how to test the DashFlow example applications.

---

## Quick Start

### Smoke Test (No API Keys Required)

Verify the librarian app compiles and can start:

```bash
cargo build --release -p librarian
./target/release/librarian --help
```

### Full E2E Tests (Requires API Keys)

Full E2E tests require an OpenAI API key:

```bash
export OPENAI_API_KEY="sk-..."
cargo test -p librarian
cargo test -p librarian -- --ignored --nocapture  # Run ignored E2E tests
```

---

## Example Apps Overview

| App | Description | E2E Tests | API Keys Required |
|-----|-------------|-----------|-------------------|
| `librarian` | Production RAG with streaming, evals, and observability | Yes | OpenAI |
| `common` | Shared utilities for examples | N/A | None |

> **Historical Note:** Previous example apps (document_search, advanced_rag, code_assistant,
> research_team, checkpoint_demo, error_recovery, streaming_aggregator, etc.) have been
> consolidated into the `librarian` paragon application. See [docs/EXAMPLE_APPS.md](../../docs/EXAMPLE_APPS.md).

---

## Environment Variables

### Required for Full E2E

- `OPENAI_API_KEY` - OpenAI API key for LLM calls

### Optional Configuration

- `OPENAI_MODEL` - Override default model (default: `gpt-4o-mini`)
- `OPENAI_BASE_URL` - Custom API endpoint
- `DASHFLOW_LOG_LEVEL` - Logging verbosity (`debug`, `info`, `warn`, `error`)

---

## Running Librarian

The librarian app has several modes:

```bash
cd apps/librarian

# Show help
cargo run --release -- --help

# Query mode - single question
cargo run --release -- query "What is async programming in Rust?"

# Interactive mode - multi-turn conversation
cargo run --release -- interactive

# With streaming (requires dashstream feature)
cargo run --release --features dashstream -- query "Explain tokio"

# Run evaluation suite
cargo run --release -- eval --suite data/eval_suite.json
```

---

## Test Categories

### 1. Unit Tests

Test individual components without external calls:

```bash
cargo test -p librarian --lib
```

### 2. Integration Tests

Test with mocked LLM responses:

```bash
cargo test -p librarian --test '*'
```

### 3. E2E Tests

Full tests with real API calls (marked `#[ignore]` by default):

```bash
export OPENAI_API_KEY="sk-..."
cargo test -p librarian -- --ignored --nocapture
```

---

## Adding Tests

When adding tests to librarian, follow this pattern:

```rust
//! E2E tests for librarian

use std::process::Command;

#[test]
fn test_help_command() {
    let output = Command::new("cargo")
        .args(["run", "-p", "librarian", "--", "--help"])
        .output()
        .expect("Failed to run app");

    assert!(output.status.success(), "Help command should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"), "Should show usage info");
}

#[tokio::test]
#[ignore = "Requires OPENAI_API_KEY"]
async fn test_basic_query() {
    if std::env::var("OPENAI_API_KEY").is_err() {
        return; // Skip if no API key
    }
    // ... test implementation
}
```

---

## Troubleshooting

### App fails to compile

1. Check that all DashFlow crates are up to date: `cargo update`
2. Ensure you're using Rust 1.80+ (`rustup update`)
3. Clear build cache: `cargo clean && cargo build`

### E2E test fails with "API key not set"

1. Verify `OPENAI_API_KEY` is exported: `echo $OPENAI_API_KEY`
2. Check the key is valid (not expired)
3. Ensure sufficient API credits

### Test times out

Some E2E tests make multiple API calls. If timeouts occur:

1. Check network connectivity
2. Increase test timeout
3. Use a faster model (`gpt-3.5-turbo` instead of `gpt-4`)

---

## Cost Considerations

Running full E2E tests with real API calls incurs costs:

- **Estimated cost per full test run:** ~$0.05-0.20
- **Most expensive tests:** Multi-turn conversation tests

To minimize costs during development:

1. Run unit tests first (free)
2. Use mocked tests where possible
3. Use `gpt-3.5-turbo` for testing (cheaper than `gpt-4`)
4. Run full E2E tests only before merge

### Cost Tracking Utilities

The `dashflow-test-utils` crate provides cost tracking and rate limiting utilities:

```rust
use dashflow_test_utils::{TestCostTracker, with_rate_limit_retry, RetryConfig};

// Track test costs with $1.00 budget
let tracker = TestCostTracker::new().with_budget(1.0);

// Record token usage after each LLM call
tracker.record_usage("gpt-4o-mini", 1000, 500);

// Check budget and warn if exceeded
tracker.check_and_warn();

// Get recommended test model
let model = dashflow_test_utils::recommended_test_model();
```

**Environment Variables:**

- `TEST_LLM_MODEL` - Override default model
- `TEST_TIMEOUT` - Override test timeout (default: 300s)

---

## See Also

- [docs/EXAMPLE_APPS.md](../../docs/EXAMPLE_APPS.md) - Detailed app descriptions
- [apps/librarian/README.md](librarian/README.md) - Librarian documentation
- [QUICKSTART.md](../../QUICKSTART.md) - Getting started guide

---

**Last Updated:** December 19, 2025
