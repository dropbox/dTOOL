#!/bin/bash
# Run TLC model checker on TLA+ specs
# Usage: ./scripts/tlc.sh <spec.tla>

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
TOOLS_DIR="$PROJECT_ROOT/tools"
TLA_DIR="$PROJECT_ROOT/tla"

# Ensure Java is in PATH
export PATH="/opt/homebrew/opt/openjdk@21/bin:$PATH"

if [ ! -f "$TOOLS_DIR/tla2tools.jar" ]; then
    echo "ERROR: tla2tools.jar not found. Run:"
    echo "  curl -L -o $TOOLS_DIR/tla2tools.jar https://github.com/tlaplus/tlaplus/releases/download/v1.8.0/tla2tools.jar"
    exit 1
fi

if [ $# -eq 0 ]; then
    echo "Usage: $0 <spec.tla> [options]"
    echo ""
    echo "Available specs:"
    ls -1 "$TLA_DIR"/*.tla 2>/dev/null | xargs -n1 basename
    exit 0
fi

SPEC="$1"
shift

# If just filename given, look in tla/ directory
if [ ! -f "$SPEC" ] && [ -f "$TLA_DIR/$SPEC" ]; then
    SPEC="$TLA_DIR/$SPEC"
fi

if [ ! -f "$SPEC" ]; then
    echo "ERROR: Spec not found: $SPEC"
    exit 1
fi

echo "Running TLC on: $SPEC"
java -XX:+UseParallelGC -cp "$TOOLS_DIR/tla2tools.jar" tlc2.TLC -deadlock "$SPEC" "$@"
