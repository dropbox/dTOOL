#!/bin/bash
# Smoke test all core features to verify they work
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

set -euo pipefail

# Navigate to repo root (script is in scripts/)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

echo "=== Smoke Testing All Core Features ==="
echo ""

# Load API key from repo .env if not already set
if [ -z "${OPENAI_API_KEY:-}" ] && [ -f "$REPO_ROOT/.env" ]; then
    export OPENAI_API_KEY=$(grep OPENAI_API_KEY "$REPO_ROOT/.env" | cut -d '=' -f 2)
fi

PASSED=0
FAILED=0

# Feature 1: bind_tools()
echo "1. Testing bind_tools()..."
if cargo test -p dashflow-openai test_tool_calling -- --ignored --test-threads=1 2>&1 | grep -q "test result: ok"; then
    echo "   ✓ bind_tools() works"
    ((PASSED++))
else
    echo "   ✗ FAILED"
    ((FAILED++))
fi

# Feature 2: create_react_agent()
echo "2. Testing create_react_agent()..."
if cargo test -p dashflow-standard-tests test_agent_loop -- --ignored --test-threads=1 2>&1 | grep -q "test result: ok"; then
    echo "   ✓ create_react_agent() works"
    ((PASSED++))
else
    echo "   ✗ FAILED"
    ((FAILED++))
fi

# Feature 3: Structured outputs
echo "3. Testing structured outputs..."
if cargo test -p dashflow test_structured_output -- --ignored --test-threads=1 2>&1 | grep -q "test result: ok"; then
    echo "   ✓ Structured outputs work"
    ((PASSED++))
else
    echo "   ✗ FAILED"
    ((FAILED++))
fi

# Feature 4: Human-in-the-loop
echo "4. Testing human-in-the-loop..."
if cargo test -p dashflow test_interrupt_before --test-threads=1 2>&1 | grep -q "test result: ok"; then
    echo "   ✓ Human-in-the-loop works"
    ((PASSED++))
else
    echo "   ✗ FAILED"
    ((FAILED++))
fi

# Feature 5: DashFlow Streaming (requires Kafka)
echo "5. Testing DashFlow Streaming..."
if brew services list | grep -q "kafka.*started"; then
    if cargo test -p dashflow --features dashstream --test dashstream_integration -- --ignored --test-threads=1 2>&1 | grep -q "test result: ok"; then
        echo "   ✓ DashFlow Streaming works (Kafka verified)"
        ((PASSED++))
    else
        echo "   ✗ FAILED"
        ((FAILED++))
    fi
else
    echo "   ⚠ Skipped (Kafka not running - run: brew services start kafka)"
fi

# Feature 6: stream_events()
echo "6. Testing stream_events()..."
if cargo test -p dashflow stream_events --test-threads=1 2>&1 | grep -q "test result: ok"; then
    echo "   ✓ stream_events() works"
    ((PASSED++))
else
    echo "   ✗ FAILED"
    ((FAILED++))
fi

# Feature 7: add_messages reducer
echo "7. Testing add_messages reducer..."
if cargo test -p dashflow add_messages --test-threads=1 2>&1 | grep -q "test result: ok"; then
    echo "   ✓ add_messages works"
    ((PASSED++))
else
    echo "   ✗ FAILED"
    ((FAILED++))
fi

# Feature 8: Apps compile
echo "8. Testing apps compile..."
if cargo build --release --bins 2>&1 | grep -q "Finished"; then
    echo "   ✓ All apps compile"
    ((PASSED++))
else
    echo "   ✗ FAILED"
    ((FAILED++))
fi

echo ""
echo "=== Results ==="
echo "Passed: $PASSED"
echo "Failed: $FAILED"
echo ""

if [ $FAILED -eq 0 ]; then
    echo "✅ All features verified working!"
    exit 0
else
    echo "⚠️  Some features failed verification"
    exit 1
fi
