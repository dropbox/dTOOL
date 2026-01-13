#!/usr/bin/env bash
# Runs traced_agent multiple times to generate continuous telemetry
# Usage: ./scripts/demo_loop.sh [iterations] [delay_seconds]

set -euo pipefail

ITERATIONS=${1:-10}
DELAY=${2:-3}

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

echo "Running traced_agent ${ITERATIONS} times with ${DELAY}s delay..."
echo "This generates live telemetry for the observability UI."
echo ""

for i in $(seq 1 $ITERATIONS); do
  echo "=== Iteration $i of $ITERATIONS ==="
  cargo run -p dashflow --example traced_agent --features observability,dashstream --release 2>&1 || {
    echo "Warning: traced_agent failed on iteration $i"
    continue
  }

  if [ "$i" -lt "$ITERATIONS" ]; then
    echo "Waiting ${DELAY}s before next run..."
    sleep "$DELAY"
  fi
  echo ""
done

echo "Demo complete! Ran $ITERATIONS iterations."
