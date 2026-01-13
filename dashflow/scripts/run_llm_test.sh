#!/bin/bash
set -euo pipefail
# Wrapper to run LLM validation tests with proper environment

# Load environment variables
if [ -f .env ]; then
    set -a
    source .env
    set +a
fi

# Run the test
python3 "$@"
