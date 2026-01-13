#!/bin/bash
#
# DashTerm2 Performance Benchmark Runner
#
# Usage:
#   ./scripts/run-benchmarks.sh [options]
#
# Options:
#   --category NAME    Run specific category (text, buffer, memory, metal)
#   --json PATH        Output results as JSON
#   --compare [PATH]   Compare against baseline
#   --save-baseline    Save results as new baseline
#   --threshold N      Regression threshold percentage (default: 10)
#   --quick            Quick run with fewer iterations
#   --list             List available benchmarks
#   --build-only       Only build, don't run
#   --help             Show help
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_DIR="$PROJECT_DIR/build/Benchmarks"
BENCHMARK_BINARY="$BUILD_DIR/DashTermBenchmarks"
BASELINES_DIR="$PROJECT_DIR/Benchmarks/baselines"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Parse arguments
BUILD_ONLY=false
BENCHMARK_ARGS=()

while [[ $# -gt 0 ]]; do
    case $1 in
        --build-only)
            BUILD_ONLY=true
            shift
            ;;
        --help|-h)
            echo "DashTerm2 Performance Benchmark Runner"
            echo ""
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --category NAME    Run specific category (text, buffer, memory, metal)"
            echo "  --json PATH        Output results as JSON"
            echo "  --compare [PATH]   Compare against baseline (uses latest if no path)"
            echo "  --save-baseline    Save results as new baseline"
            echo "  --threshold N      Regression threshold percentage (default: 10)"
            echo "  --quick            Quick run with fewer iterations"
            echo "  --list             List available benchmarks"
            echo "  --build-only       Only build, don't run"
            echo "  --help             Show this help"
            exit 0
            ;;
        --compare)
            if [[ $# -gt 1 && ! "$2" =~ ^-- ]]; then
                BENCHMARK_ARGS+=("--compare" "$2")
                shift 2
            else
                # Find latest baseline
                if [[ -d "$BASELINES_DIR" ]]; then
                    LATEST_BASELINE=$(ls -t "$BASELINES_DIR"/*.json 2>/dev/null | head -1)
                    if [[ -n "$LATEST_BASELINE" ]]; then
                        echo -e "${BLUE}Using latest baseline: $LATEST_BASELINE${NC}"
                        BENCHMARK_ARGS+=("--compare" "$LATEST_BASELINE")
                    else
                        echo -e "${YELLOW}Warning: No baseline found in $BASELINES_DIR${NC}"
                    fi
                fi
                shift
            fi
            ;;
        --save-baseline)
            TIMESTAMP=$(date +%Y%m%d_%H%M%S)
            BASELINE_PATH="$BASELINES_DIR/baseline_$TIMESTAMP.json"
            BENCHMARK_ARGS+=("--save-baseline" "$BASELINE_PATH")
            shift
            ;;
        *)
            BENCHMARK_ARGS+=("$1")
            shift
            ;;
    esac
done

echo -e "${BLUE}╔═══════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║           DashTerm2 Performance Benchmark Suite                   ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Create build directory
mkdir -p "$BUILD_DIR"

# Build benchmark executable
echo -e "${BLUE}Building benchmark suite...${NC}"

SOURCES=(
    "$PROJECT_DIR/Benchmarks/Sources/BenchmarkProtocol.swift"
    "$PROJECT_DIR/Benchmarks/Sources/BenchmarkRunner.swift"
    "$PROJECT_DIR/Benchmarks/Sources/TextBenchmarks.swift"
    "$PROJECT_DIR/Benchmarks/Sources/ScreenBufferBenchmarks.swift"
    "$PROJECT_DIR/Benchmarks/Sources/MemoryBenchmarks.swift"
    "$PROJECT_DIR/Benchmarks/Sources/MetalBenchmarks.swift"
    "$PROJECT_DIR/Benchmarks/Sources/main.swift"
)

# Check all source files exist
for src in "${SOURCES[@]}"; do
    if [[ ! -f "$src" ]]; then
        echo -e "${RED}Error: Source file not found: $src${NC}"
        exit 1
    fi
done

# Compile with swiftc
swiftc \
    -O \
    -whole-module-optimization \
    -target arm64-apple-macosx14.0 \
    -sdk "$(xcrun --show-sdk-path)" \
    -o "$BENCHMARK_BINARY" \
    "${SOURCES[@]}" \
    2>&1

if [[ $? -ne 0 ]]; then
    echo -e "${RED}Build failed!${NC}"
    exit 1
fi

echo -e "${GREEN}Build successful: $BENCHMARK_BINARY${NC}"
echo ""

if [[ "$BUILD_ONLY" == "true" ]]; then
    echo "Build-only mode, skipping execution."
    exit 0
fi

# Run benchmarks
echo -e "${BLUE}Running benchmarks...${NC}"
echo ""

# Create baselines directory if saving baseline
if [[ " ${BENCHMARK_ARGS[*]} " =~ " --save-baseline " ]]; then
    mkdir -p "$BASELINES_DIR"
fi

# Execute benchmarks
"$BENCHMARK_BINARY" "${BENCHMARK_ARGS[@]}"
EXIT_CODE=$?

# Report exit status
echo ""
case $EXIT_CODE in
    0)
        echo -e "${GREEN}✓ Benchmarks completed successfully${NC}"
        ;;
    1)
        echo -e "${RED}✗ Benchmark execution failed${NC}"
        ;;
    2)
        echo -e "${YELLOW}⚠ Performance regression detected${NC}"
        ;;
    *)
        echo -e "${RED}✗ Unexpected error (exit code: $EXIT_CODE)${NC}"
        ;;
esac

exit $EXIT_CODE
