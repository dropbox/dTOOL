#!/bin/bash
# Parse DashFlow Streaming events from Kafka
# Wrapper script for the parse_events binary
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

# Build if needed
if [ ! -f "target/debug/parse_events" ]; then
    echo "Building parse_events binary..."
    cargo build --bin parse_events
fi

# Run with all arguments passed through
exec target/debug/parse_events "$@"
