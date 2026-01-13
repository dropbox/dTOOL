#!/bin/bash
# Run all hot path benchmarks for DashFlow
# Portable across macOS and Linux
#
# Usage:
#   ./scripts/run_hot_path_benchmarks.sh           # Run all
#   ./scripts/run_hot_path_benchmarks.sh --quick   # Run reduced set
#
# See docs/BENCHMARK_RUNBOOK.md for interpretation guidance.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_ROOT"

# Detect OS for portable open command
OS="$(uname -s)"
case "$OS" in
    Darwin) OPEN_CMD="open" ;;
    Linux)  OPEN_CMD="xdg-open" ;;
    *)      OPEN_CMD="echo" ;;  # Fallback: just print the path
esac

QUICK_MODE=false
if [[ "${1:-}" == "--quick" ]]; then
    QUICK_MODE=true
fi

echo "=============================================="
echo "DashFlow Hot Path Benchmark Suite"
echo "=============================================="
echo "Start time: $(date)"
echo "Mode: $(if $QUICK_MODE; then echo 'Quick'; else echo 'Full'; fi)"
echo ""

# Ensure release build
echo "Building release binaries..."
cargo build --release -p dashflow -p dashflow-streaming -p dashflow-benchmarks 2>/dev/null

echo ""
echo "=============================================="
echo "1/4: Graph Executor Benchmarks"
echo "=============================================="
if $QUICK_MODE; then
    # Quick mode: only core benchmarks
    cargo bench -p dashflow --bench graph_benchmarks -- \
        "compilation/simple_graph_3_nodes" \
        "sequential_execution/3_nodes_simple" \
        "parallel_execution/fanout_3_workers" \
        "checkpointing/memory_checkpoint_3_nodes" \
        --sample-size 20
else
    cargo bench -p dashflow --bench graph_benchmarks
fi

echo ""
echo "=============================================="
echo "2/4: Streaming Codec Benchmarks"
echo "=============================================="
if $QUICK_MODE; then
    cargo bench -p dashflow-streaming --bench codec_benchmarks -- \
        "encode/event" \
        "decode/event" \
        "roundtrip/event" \
        --sample-size 20
else
    cargo bench -p dashflow-streaming --bench codec_benchmarks
    cargo bench -p dashflow-streaming --bench diff_benchmarks
fi

echo ""
echo "=============================================="
echo "3/4: Vector Store Benchmarks"
echo "=============================================="
if $QUICK_MODE; then
    cargo bench -p dashflow-benchmarks --bench vectorstore_benchmarks -- \
        "vectorstore_add/100" \
        "similarity_search/search_1000_docs_k5" \
        --sample-size 10
else
    cargo bench -p dashflow-benchmarks --bench vectorstore_benchmarks
fi

echo ""
echo "=============================================="
echo "4/4: Registry Client Benchmarks"
echo "=============================================="
if $QUICK_MODE; then
    cargo bench -p dashflow-benchmarks --bench registry_benchmarks -- \
        "registry_hashing/sha256_1mb" \
        "registry_manifest_serialize/medium_to_json" \
        --sample-size 20
else
    cargo bench -p dashflow-benchmarks --bench registry_benchmarks
fi

echo ""
echo "=============================================="
echo "Benchmark Suite Complete"
echo "=============================================="
echo "End time: $(date)"
echo ""
echo "Results saved to: target/criterion/"
echo "View report: $OPEN_CMD target/criterion/report/index.html"
echo ""
echo "For regression analysis, see: docs/BENCHMARK_RUNBOOK.md"
