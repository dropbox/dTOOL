#!/bin/bash
# cargo_check_lint.sh - Comprehensive check: cargo check + clippy + platform lint
#
# Runs all checks in sequence, stopping on first failure.
# This is the recommended way to verify code before committing.
#
# Usage:
#   ./scripts/cargo_check_lint.sh           # Check entire workspace
#   ./scripts/cargo_check_lint.sh src/      # Check specific directory
#   ./scripts/cargo_check_lint.sh --quick   # Skip lint (faster)
#   ./scripts/cargo_check_lint.sh --lint-only  # Just run lint
#
# Options:
#   --quick     Skip platform lint (faster but less thorough)
#   --lint-only Skip cargo checks, run only platform lint
#   --no-clippy Skip clippy warnings
#   --strict    Treat warnings as errors + forbid unwrap/expect in prod targets (allowlist via #[allow(clippy::unwrap_used|expect_used)])
#   -v          Verbose output
#
# Exit codes:
#   0 - All checks passed
#   1 - cargo check failed
#   2 - clippy failed
#   3 - platform lint failed

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Parse options
QUICK=false
LINT_ONLY=false
NO_CLIPPY=false
STRICT=false
VERBOSE=false
LINT_PATH="."

while [[ $# -gt 0 ]]; do
    case $1 in
        --quick)
            QUICK=true
            shift
            ;;
        --lint-only)
            LINT_ONLY=true
            shift
            ;;
        --no-clippy)
            NO_CLIPPY=true
            shift
            ;;
        --strict)
            STRICT=true
            shift
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        *)
            LINT_PATH="$1"
            shift
            ;;
    esac
done

cd "$REPO_ROOT"

# Helper function for verbose output
log() {
    if [[ "$VERBOSE" == "true" ]]; then
        echo "[INFO] $*"
    fi
}

# Step 1: Cargo check
if [[ "$LINT_ONLY" != "true" ]]; then
    echo "=== Running cargo check ==="
    log "Checking all workspace crates..."
    if ! cargo check --workspace; then
        echo "FAILED: cargo check found errors"
        exit 1
    fi
    echo "cargo check: OK"
    echo
fi

# Step 2: Clippy (optional)
if [[ "$LINT_ONLY" != "true" && "$NO_CLIPPY" != "true" ]]; then
    echo "=== Running clippy ==="
    # Always run a broad clippy pass to catch compilation issues across targets.
    log "Running: cargo clippy --workspace --all-targets"
    if ! cargo clippy --workspace --all-targets; then
        echo "FAILED: clippy found issues"
        exit 2
    fi

    # M-294: Prevent new production unwrap()/expect() usage.
    # This pass is intentionally narrower (lib + bins) so test code isn't blocked.
    log "Running: cargo clippy --workspace --lib --bins -- -D clippy::unwrap_used -D clippy::expect_used"
    if ! cargo clippy --workspace --lib --bins -- -D clippy::unwrap_used -D clippy::expect_used; then
        echo "FAILED: unwrap/expect not allowed in prod targets (add #[allow(clippy::unwrap_used|expect_used)] with SAFETY justification if intentional)"
        exit 2
    fi

    # In strict mode, enforce zero warnings on production targets (lib + bins).
    if [[ "$STRICT" == "true" ]]; then
        log "Running: cargo clippy --workspace --lib --bins -- -D warnings -D clippy::unwrap_used -D clippy::expect_used"
        if ! cargo clippy --workspace --lib --bins -- -D warnings -D clippy::unwrap_used -D clippy::expect_used; then
            echo "FAILED: clippy found issues (prod targets, strict)"
            exit 2
        fi
    fi
    echo "clippy: OK"
    echo
fi

# Step 3: Platform usage lint (unless --quick)
if [[ "$QUICK" != "true" ]]; then
    echo "=== Running platform usage linter ==="

    # Build CLI if needed
    log "Building dashflow-cli..."
    cargo build -p dashflow-cli --release -q 2>/dev/null || cargo build -p dashflow-cli -q

    # Find binary
    if [[ -f "$REPO_ROOT/target/release/dashflow" ]]; then
        DASHFLOW="$REPO_ROOT/target/release/dashflow"
    else
        DASHFLOW="$REPO_ROOT/target/debug/dashflow"
    fi

    LINT_ARGS="$LINT_PATH"
    if [[ "$VERBOSE" == "true" ]]; then
        LINT_ARGS="$LINT_ARGS --explain"
    fi

    # Platform lint is advisory by default (don't fail on warnings)
    log "Running: $DASHFLOW lint $LINT_ARGS"
    "$DASHFLOW" lint $LINT_ARGS || {
        LINT_EXIT=$?
        if [[ "$STRICT" == "true" && $LINT_EXIT -ne 0 ]]; then
            echo "FAILED: platform lint found issues"
            exit 3
        fi
        echo "lint: WARNINGS (non-fatal)"
    }
    echo "lint: OK"
    echo
fi

echo "=== All checks passed ==="
