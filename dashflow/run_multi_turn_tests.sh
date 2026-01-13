#!/bin/bash
# Script to run multi-turn conversation tests with OPENAI_API_KEY from .env
# Updated: 2025-12-19 - Example apps consolidated to librarian

set -euo pipefail

# Load .env file
if [ -f .env ]; then
    OPENAI_API_KEY="$(
        sed -nE 's/^(export[[:space:]]+)?OPENAI_API_KEY[[:space:]]*=[[:space:]]*"?([^"#]*)"?([[:space:]]*#.*)?$/\2/p' .env | tail -n 1
    )"
    export OPENAI_API_KEY
fi

# Verify API key is set
if [ -z "${OPENAI_API_KEY:-}" ]; then
    echo "ERROR: OPENAI_API_KEY not found in .env"
    exit 1
fi

echo "Running multi-turn conversation tests..."
echo "================================================"

echo ""
echo "1. Librarian Tests"
echo "------------------"
cargo test --package librarian -- --ignored --nocapture --test-threads=1

echo ""
echo "================================================"
echo "All multi-turn conversation tests complete"
