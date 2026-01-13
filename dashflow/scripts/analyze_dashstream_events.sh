#!/usr/bin/env bash
# Convenience wrapper for analyze_events binary
# Builds the binary if needed and runs it with arguments
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Build binary if needed
if [ ! -f "$PROJECT_ROOT/target/debug/analyze_events" ]; then
    echo "Building analyze_events binary..." >&2
    cargo build --bin analyze_events --manifest-path "$PROJECT_ROOT/Cargo.toml"
fi

# Run with all arguments passed through
exec "$PROJECT_ROOT/target/debug/analyze_events" "$@"
