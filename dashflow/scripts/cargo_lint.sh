#!/bin/bash
# cargo_lint.sh - Run DashFlow platform usage linter
#
# This script provides cargo-style integration for the platform usage linter.
# It runs the lint command after building the CLI.
#
# Usage:
#   ./scripts/cargo_lint.sh [path] [options]
#   ./scripts/cargo_lint.sh examples/apps/librarian --explain
#   ./scripts/cargo_lint.sh . --format json
#
# Options:
#   All options are passed through to `dashflow lint`
#
# Environment:
#   DASHFLOW_LINT_SEVERITY - default severity (info, warn, error)
#   DASHFLOW_LINT_FORMAT   - default format (text, json, sarif)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Default values from environment or sensible defaults
SEVERITY="${DASHFLOW_LINT_SEVERITY:-warn}"
FORMAT="${DASHFLOW_LINT_FORMAT:-text}"

# Check if we're in the DashFlow repo
if [[ ! -f "$REPO_ROOT/Cargo.toml" ]]; then
    echo "Error: Must be run from DashFlow repository root"
    exit 1
fi

# Build the CLI if needed (release mode for speed)
echo "Building dashflow-cli..."
cargo build -p dashflow-cli --release -q 2>/dev/null || {
    echo "Building in dev mode..."
    cargo build -p dashflow-cli -q
}

# Find the binary
if [[ -f "$REPO_ROOT/target/release/dashflow" ]]; then
    DASHFLOW="$REPO_ROOT/target/release/dashflow"
elif [[ -f "$REPO_ROOT/target/debug/dashflow" ]]; then
    DASHFLOW="$REPO_ROOT/target/debug/dashflow"
else
    echo "Error: Could not find dashflow binary"
    exit 1
fi

# Run the lint command
echo "Running platform usage linter..."
"$DASHFLOW" lint "$@"
