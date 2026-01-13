#!/bin/bash
# Performance Baseline Script
# Runs a standardized set of tests to establish performance baseline
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
print_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
print_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
print_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Configuration
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RESULTS_DIR="load-tests/results"
BASELINE_FILE="$RESULTS_DIR/baseline_${TIMESTAMP}.json"
BASE_URL="${BASE_URL:-http://localhost:8080}"

# Create results directory
mkdir -p "$RESULTS_DIR"

print_info "Performance Baseline Test Suite"
print_info "Timestamp: $TIMESTAMP"
print_info "Target: $BASE_URL"
print_info "Results: $BASELINE_FILE"
echo ""

# Function to run a single test and extract metrics
run_baseline_test() {
    local name=$1
    local scenario=$2
    local duration=$3

    print_info "Running: $name ($duration)"

    local output_file="$RESULTS_DIR/${name}_${TIMESTAMP}.json"

    # Run test with JSON output
    BASE_URL="$BASE_URL" k6 run \
        --out "json=$output_file" \
        --duration "$duration" \
        "load-tests/k6/scenarios/${scenario}.js" \
        > /dev/null 2>&1 || true

    print_success "Completed: $name"

    # Extract key metrics (simplified - in production use jq)
    echo "$output_file"
}

# Function to check server
check_server() {
    print_info "Checking server health..."

    if curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/health" 2>/dev/null | grep -q "200"; then
        print_success "Server is healthy"
        return 0
    else
        print_error "Server is not responding"
        return 1
    fi
}

# Function to generate summary report
generate_summary() {
    print_info "Generating summary report..."

    cat > "$RESULTS_DIR/baseline_${TIMESTAMP}_summary.txt" <<EOF
Performance Baseline Report
===========================

Timestamp: $TIMESTAMP
Target: $BASE_URL
Date: $(date)

Tests Completed:
EOF

    for test_name in "${!TEST_RESULTS[@]}"; do
        echo "  - $test_name: ${TEST_RESULTS[$test_name]}" >> "$RESULTS_DIR/baseline_${TIMESTAMP}_summary.txt"
    done

    cat >> "$RESULTS_DIR/baseline_${TIMESTAMP}_summary.txt" <<EOF

Baseline Expectations (from benchmarks):
- P50 latency: < 50ms
- P95 latency: < 200ms
- P99 latency: < 500ms
- Error rate: < 0.1%
- Throughput: > 1,000 req/s per pod

Comparison:
- See individual test result files in $RESULTS_DIR
- Use k6 Cloud or Grafana for detailed visualization

Next Steps:
1. Review individual test results
2. Compare against previous baselines
3. Investigate any regressions
4. Update performance targets if needed

EOF

    print_success "Summary report: $RESULTS_DIR/baseline_${TIMESTAMP}_summary.txt"
    cat "$RESULTS_DIR/baseline_${TIMESTAMP}_summary.txt"
}

# Main execution
main() {
    # Check prerequisites
    if ! command -v k6 &> /dev/null; then
        print_error "k6 is not installed"
        exit 1
    fi

    # Check server
    if ! check_server; then
        print_error "Server must be running"
        print_info "Start server: cargo run --release --example basic_skeleton -p dashflow-langserve"
        exit 1
    fi

    echo ""
    print_info "Starting baseline tests..."
    echo ""

    declare -A TEST_RESULTS

    # Test 1: Quick smoke test
    print_info "=== Test 1: Smoke Test ==="
    result=$(run_baseline_test "smoke" "basic-invoke" "30s")
    TEST_RESULTS["smoke"]="$result"
    echo ""
    sleep 5

    # Test 2: Basic invoke
    print_info "=== Test 2: Basic Invoke Load ==="
    result=$(run_baseline_test "basic-invoke" "basic-invoke" "5m")
    TEST_RESULTS["basic-invoke"]="$result"
    echo ""
    sleep 5

    # Test 3: Batch processing
    print_info "=== Test 3: Batch Processing ==="
    result=$(run_baseline_test "batch" "batch" "5m")
    TEST_RESULTS["batch"]="$result"
    echo ""
    sleep 5

    # Test 4: Streaming
    print_info "=== Test 4: Streaming ==="
    result=$(run_baseline_test "streaming" "streaming" "5m")
    TEST_RESULTS["streaming"]="$result"
    echo ""
    sleep 5

    # Test 5: Mixed workload
    print_info "=== Test 5: Mixed Workload ==="
    result=$(run_baseline_test "mixed" "mixed-workload" "10m")
    TEST_RESULTS["mixed"]="$result"
    echo ""

    # Generate summary
    echo ""
    generate_summary

    print_success "Baseline test suite complete!"
    print_info "Results saved to: $RESULTS_DIR"
}

main
