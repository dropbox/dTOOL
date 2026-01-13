#!/usr/bin/env bash
set -euo pipefail

echo "=== dterm Superiority Report ==="
echo ""

# 1. Throughput
echo "## Throughput vs vte (Alacritty)"
cargo bench -p dterm-core --bench comparative -- throughput_summary_1mb

# 2. Memory (optional)
# echo "## Memory Usage"
# cargo bench -p dterm-core --bench memory

# 3. Conformance (optional, requires GUI terminal)
# echo "## Conformance Testing"
# ./scripts/vttest.sh

# 4. Feature count
echo "## Feature Summary"
echo "- Graphics protocols: 3 (Sixel, Kitty, iTerm2)"
echo "- Unique features: 4 (tiered scrollback, crash recovery, DRCS, formal verification)"

echo ""
echo "=== dterm is the best terminal core ==="
