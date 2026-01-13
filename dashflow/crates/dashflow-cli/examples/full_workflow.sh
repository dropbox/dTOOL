#!/bin/bash
#
# DashFlow Streaming CLI - Full Workflow Demo
#
# This script demonstrates all 8 DashFlow Streaming CLI commands in a complete workflow,
# showing how to monitor, debug, and analyze DashFlow execution streams.
#
# Prerequisites:
# - Kafka running on localhost:9092
# - DashFlow application with DashFlow Streaming telemetry enabled
# - dashflow CLI installed
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
KAFKA_BROKERS="${DASHFLOW_KAFKA_BROKERS:-localhost:9092}"
TOPIC="${DASHFLOW_KAFKA_TOPIC:-dashstream-events}"
OUTPUT_DIR="./dashflow-demo-output"

# Helper functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

separator() {
    echo -e "\n${BLUE}========================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}========================================${NC}\n"
}

# Check prerequisites
check_prerequisites() {
    separator "Checking Prerequisites"

    # Check if dashflow CLI is installed
    if ! command -v dashflow &> /dev/null; then
        log_error "dashflow CLI not found. Please install with:"
        echo "  cargo install --path crates/dashflow-cli"
        exit 1
    fi
    log_success "dashflow CLI found"

    # Check if Kafka is accessible
    if ! nc -z localhost 9092 2>/dev/null; then
        log_warn "Kafka not accessible at localhost:9092"
        log_warn "Start Kafka with: docker-compose -f docker-compose-kafka.yml up -d"
        exit 1
    fi
    log_success "Kafka accessible at $KAFKA_BROKERS"

    # Create output directory
    mkdir -p "$OUTPUT_DIR"
    log_success "Output directory: $OUTPUT_DIR"
}

# 1. Tail: Stream live events
demo_tail() {
    separator "1. TAIL - Stream Live Events"

    log_info "Streaming last 10 events from topic '$TOPIC'..."
    dashflow tail \
        --bootstrap-servers "$KAFKA_BROKERS" \
        --topic "$TOPIC" \
        --from-beginning \
        --limit 10 \
        --follow false \
        2>/dev/null || log_warn "No events found or Kafka not ready"

    log_success "Tail command completed"

    # Provide a thread ID explicitly (e.g., from your app's logs) via DASHFLOW_THREAD_ID.
    THREAD_ID="${DASHFLOW_THREAD_ID:-}"
    if [ -z "$THREAD_ID" ]; then
        log_warn "DASHFLOW_THREAD_ID not set; using example thread ID for demo purposes"
        THREAD_ID="demo-session-abc123"
    fi
    log_success "Using thread ID: $THREAD_ID"
}

# 2. Inspect: Show thread details
demo_inspect() {
    separator "2. INSPECT - Thread Details & History"

    log_info "Inspecting thread: $THREAD_ID"
    dashflow inspect \
        --bootstrap-servers "$KAFKA_BROKERS" \
        --topic "$TOPIC" \
        --thread "$THREAD_ID" \
        --stats \
        2>/dev/null || log_warn "Thread not found or no events available"

    log_success "Inspect command completed"

    # Show detailed view
    log_info "Showing detailed event log..."
    dashflow inspect \
        --bootstrap-servers "$KAFKA_BROKERS" \
        --topic "$TOPIC" \
        --thread "$THREAD_ID" \
        --detailed \
        2>/dev/null | head -20 || true

    log_success "Detailed inspect completed"
}

# 3. Export: Export thread data
demo_export() {
    separator "3. EXPORT - Export Thread Data to JSON"

    local output_file="$OUTPUT_DIR/thread_${THREAD_ID}.json"

    log_info "Exporting thread '$THREAD_ID' to: $output_file"
    dashflow export \
        --bootstrap-servers "$KAFKA_BROKERS" \
        --topic "$TOPIC" \
        --thread "$THREAD_ID" \
        --output "$output_file" \
        --pretty \
        2>/dev/null || log_warn "Export failed - thread may not exist"

    if [ -f "$output_file" ]; then
        log_success "Export completed: $(wc -l < "$output_file") lines"
        log_info "Preview (first 10 lines):"
        head -10 "$output_file" | sed 's/^/  /'
    fi
}

# 4. Profile: Performance profiling
demo_profile() {
    separator "4. PROFILE - Execution Performance Profiling"

    log_info "Profiling thread: $THREAD_ID"
    dashflow profile \
        --bootstrap-servers "$KAFKA_BROKERS" \
        --topic "$TOPIC" \
        --thread "$THREAD_ID" \
        --detailed \
        2>/dev/null || log_warn "Profile failed - thread may not exist"

    log_success "Profile command completed"
}

# 5. Flamegraph: Performance visualization
demo_flamegraph() {
    separator "5. FLAMEGRAPH - Performance Visualization"

    local output_file="$OUTPUT_DIR/flamegraph_${THREAD_ID}.svg"

    log_info "Generating flamegraph for thread: $THREAD_ID"
    log_info "Output: $output_file"

    dashflow flamegraph \
        --bootstrap-servers "$KAFKA_BROKERS" \
        --topic "$TOPIC" \
        --thread "$THREAD_ID" \
        --output "$output_file" \
        2>/dev/null || log_warn "Flamegraph generation failed - thread may not exist"

    if [ -f "$output_file" ]; then
        log_success "Flamegraph generated: $(du -h "$output_file" | cut -f1)"
        log_info "Open in browser: file://$(realpath "$output_file")"
    fi
}

# 6. Costs: Token cost analysis
demo_costs() {
    separator "6. COSTS - Token Cost Analysis"

    log_info "Analyzing token costs..."
    dashflow costs \
        --bootstrap-servers "$KAFKA_BROKERS" \
        --topic "$TOPIC" \
        2>/dev/null || log_warn "No cost data available"

    log_success "Overall cost analysis completed"

    # By node
    log_info "Cost breakdown by node..."
    dashflow costs \
        --bootstrap-servers "$KAFKA_BROKERS" \
        --topic "$TOPIC" \
        --by-node \
        2>/dev/null || true

    log_success "Node-level cost analysis completed"

    # Save output to a file for later inspection
    local costs_file="$OUTPUT_DIR/costs.txt"
    log_info "Saving costs output to: $costs_file"
    dashflow costs \
        --bootstrap-servers "$KAFKA_BROKERS" \
        --topic "$TOPIC" \
        --by-node \
        2>/dev/null > "$costs_file" || log_warn "Cost export failed"

    [ -f "$costs_file" ] && log_success "Saved: $costs_file"
}

# 7. Replay: Time-travel debugging
demo_replay() {
    separator "7. TIMELINE REPLAY - Time-Travel Debugging"

    log_info "Replaying execution for thread: $THREAD_ID"
    dashflow timeline replay \
        --bootstrap-servers "$KAFKA_BROKERS" \
        --topic "$TOPIC" \
        --thread "$THREAD_ID" \
        2>/dev/null | head -50 || log_warn "Replay failed - thread may not exist"

    log_success "Replay command completed"
}

# 8. Diff: Compare checkpoints
demo_diff() {
    separator "8. DIFF - Compare Checkpoints"

    log_info "Finding checkpoints for thread: $THREAD_ID"

    # This would normally use actual checkpoint IDs
    # For demo, we show the command structure
    log_warn "Checkpoint diffing requires checkpoint IDs"
    log_info "Example usage:"
    echo "  dashflow diff \\"
    echo "    --bootstrap-servers $KAFKA_BROKERS \\"
    echo "    --topic $TOPIC \\"
    echo "    --thread $THREAD_ID \\"
    echo "    --checkpoint1 chk-001 \\"
    echo "    --checkpoint2 chk-002 \\"
    echo "    --detailed"

    log_success "Diff command demo completed"
}

# Summary and next steps
show_summary() {
    separator "Demo Complete - Summary"

    log_success "All 8 DashFlow Streaming CLI commands demonstrated!"

    echo ""
    echo "Commands demonstrated:"
    echo "  1. ✓ tail       - Stream live events"
    echo "  2. ✓ inspect    - Thread details & history"
    echo "  3. ✓ export     - Export to JSON"
    echo "  4. ✓ profile    - Performance profiling"
    echo "  5. ✓ flamegraph - Performance visualization"
    echo "  6. ✓ costs      - Token cost analysis"
    echo "  7. ✓ timeline replay - Time-travel debugging"
    echo "  8. ✓ diff       - Compare checkpoints"

    echo ""
    echo "Generated files:"
    if [ -d "$OUTPUT_DIR" ]; then
        ls -lh "$OUTPUT_DIR" | tail -n +2 | sed 's/^/  /'
    fi

    echo ""
    log_info "Next Steps:"
    echo "  - Explore generated files in: $OUTPUT_DIR"
    echo "  - Try different command options (see --help)"
    echo "  - Run with your own thread IDs"
    echo "  - Check out other examples:"
    echo "    - cost_analysis.sh"
    echo "    - performance_debug.sh"

    echo ""
    log_info "Documentation: crates/dashflow-cli/README.md"
}

# Main execution
main() {
    echo ""
    log_info "DashFlow Streaming CLI - Full Workflow Demo"
    log_info "===================================="
    echo ""

    check_prerequisites

    demo_tail
    demo_inspect
    demo_export
    demo_profile
    demo_flamegraph
    demo_costs
    demo_replay
    demo_diff

    show_summary
}

# Run main function
main "$@"
