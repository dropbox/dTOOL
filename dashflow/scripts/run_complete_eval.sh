#!/bin/bash
# Run complete eval loop and generate report
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
#
# This script runs the comprehensive evaluation loop test that:
# - Executes a multi-turn conversation
# - Captures all DashFlow Streaming events to Kafka
# - Evaluates output quality
# - Generates verification artifacts
#
# Prerequisites:
# - OPENAI_API_KEY environment variable
# - Kafka running on localhost:9092
#
# Usage: ./scripts/run_complete_eval.sh

set -euo pipefail

echo "=== Running Complete Evaluation Loop ==="
echo ""

# Verify prerequisites
echo "Checking prerequisites..."

# Check for OPENAI_API_KEY
if [ -z "${OPENAI_API_KEY:-}" ]; then
    if [ -f ".env" ]; then
        echo "Loading OPENAI_API_KEY from .env file..."
        export OPENAI_API_KEY=$(grep OPENAI_API_KEY .env | cut -d '=' -f 2)
    fi
fi

if [ -z "${OPENAI_API_KEY:-}" ]; then
    echo "❌ OPENAI_API_KEY not set"
    echo "   Set with: export OPENAI_API_KEY=sk-..."
    echo "   Or create .env file with: OPENAI_API_KEY=sk-..."
    exit 1
fi

# Check for Kafka (basic connectivity test)
if command -v brew &> /dev/null; then
    if brew services list | grep -q "kafka.*started"; then
        echo "✓ Kafka is running (via Homebrew)"
    else
        echo "⚠️  Kafka not detected via Homebrew services"
        echo "   If using Docker, ensure Kafka is running on localhost:9092"
        echo "   Start with: docker-compose -f docker-compose-kafka.yml up -d"
    fi
elif command -v docker &> /dev/null; then
    if docker ps | grep -q kafka; then
        echo "✓ Kafka container is running"
    else
        echo "⚠️  Kafka container not detected"
        echo "   Start with: docker-compose -f docker-compose-kafka.yml up -d"
        read -p "   Continue anyway? (y/N) " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
    fi
else
    echo "⚠️  Cannot verify Kafka status (no brew or docker found)"
    echo "   Ensure Kafka is running on localhost:9092"
    read -p "   Continue anyway? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

echo "✓ Prerequisites checked"
echo ""

# Run eval
echo "Running evaluation test..."
echo ""

cargo test --package dashflow-standard-tests \
    --test complete_eval_loop \
    --features dashstream \
    -- --ignored --nocapture

# Check if passed
if [ $? -eq 0 ]; then
    echo ""
    echo "✅ Complete eval loop PASSED"
    echo ""

    # Show artifacts
    if [ -d "eval_outputs" ]; then
        echo "Verification artifacts:"
        ls -lh eval_outputs/
        echo ""
        echo "Review:"
        echo "  - eval_outputs/conversation.txt (what was said)"
        echo "  - eval_outputs/eval_report.md (quality scores)"
        echo ""

        # Show summary from report if it exists
        if [ -f "eval_outputs/eval_report.md" ]; then
            echo "=== Evaluation Summary ==="
            cat eval_outputs/eval_report.md
        fi
    else
        echo "Note: eval_outputs/ directory not found"
        echo "Artifacts may have been created in test working directory"
    fi
else
    echo ""
    echo "❌ Complete eval loop FAILED"
    echo ""
    echo "Troubleshooting:"
    echo "  1. Check OPENAI_API_KEY is valid"
    echo "  2. Verify Kafka is running: netstat -an | grep 9092"
    echo "  3. Check test logs above for specific errors"
    echo "  4. Ensure network connectivity to OpenAI API"
    exit 1
fi
