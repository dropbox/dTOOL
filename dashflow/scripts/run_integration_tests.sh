#!/bin/bash
# Phase 494: Run integration tests with docker-compose infrastructure
#
# This script:
# 1. Starts docker-compose.test.yml services
# 2. Waits for services to be healthy
# 3. Runs integration tests (marked with #[ignore])
# 4. Optionally stops services after tests
#
# Usage:
#   ./scripts/run_integration_tests.sh              # Run all integration tests
#   ./scripts/run_integration_tests.sh --keep       # Keep services running after tests
#   ./scripts/run_integration_tests.sh --filter X   # Run only tests matching X

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
COMPOSE_FILE="$PROJECT_ROOT/docker-compose.test.yml"

# Parse arguments
KEEP_SERVICES=false
FILTER=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --keep)
            KEEP_SERVICES=true
            shift
            ;;
        --filter)
            FILTER="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--keep] [--filter PATTERN]"
            exit 1
            ;;
    esac
done

echo "╔══════════════════════════════════════════════════════════════╗"
echo "║          Integration Test Runner                             ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""

# Check docker is available
if ! command -v docker &> /dev/null; then
    echo "ERROR: docker is not installed or not in PATH"
    exit 1
fi

if ! docker info &> /dev/null; then
    echo "ERROR: Docker daemon is not running"
    exit 1
fi

# Start services
echo "📦 Starting test infrastructure..."
cd "$PROJECT_ROOT"
docker compose -f "$COMPOSE_FILE" up -d

# Wait for services to be healthy
echo ""
echo "⏳ Waiting for services to be healthy (max 60s)..."

MAX_WAIT=60
WAITED=0
INTERVAL=5

while [ $WAITED -lt $MAX_WAIT ]; do
    # Check if all services are healthy
    UNHEALTHY=$(docker compose -f "$COMPOSE_FILE" ps --format json 2>/dev/null | grep -c '"Health":"starting"' || echo "0")

    if [ "$UNHEALTHY" = "0" ]; then
        echo "✅ All services are healthy!"
        break
    fi

    echo "   Waiting for services... ($WAITED/${MAX_WAIT}s)"
    sleep $INTERVAL
    WAITED=$((WAITED + INTERVAL))
done

if [ $WAITED -ge $MAX_WAIT ]; then
    echo "⚠️  Timeout waiting for services. Some tests may fail."
fi

# Run tests
echo ""
echo "🧪 Running integration tests..."
echo ""

TEST_ARGS="--include-ignored"
if [ -n "$FILTER" ]; then
    TEST_ARGS="$TEST_ARGS $FILTER"
fi

# Run with nextest if available, otherwise cargo test
if command -v cargo-nextest &> /dev/null; then
    cargo nextest run $TEST_ARGS || TEST_EXIT=$?
else
    cargo test --workspace -- $TEST_ARGS || TEST_EXIT=$?
fi

echo ""

# Cleanup
if [ "$KEEP_SERVICES" = false ]; then
    echo "🧹 Stopping test infrastructure..."
    docker compose -f "$COMPOSE_FILE" down
else
    echo "ℹ️  Services left running (--keep). Stop with:"
    echo "   docker compose -f docker-compose.test.yml down"
fi

echo ""
if [ "${TEST_EXIT:-0}" -eq 0 ]; then
    echo "✅ Integration tests completed successfully!"
else
    echo "❌ Some integration tests failed (exit code: $TEST_EXIT)"
    exit $TEST_EXIT
fi
