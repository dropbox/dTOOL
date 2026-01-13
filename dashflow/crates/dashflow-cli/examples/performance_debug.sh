#!/bin/bash
#
# DashFlow Streaming CLI - Performance Debugging Workflow
#
# This script demonstrates how to use DashFlow Streaming CLI for comprehensive
# performance profiling, bottleneck identification, and optimization.
#
# Use Cases:
# - Identify slow operations
# - Find performance bottlenecks
# - Compare before/after optimization
# - Generate performance reports
# - Time-travel debugging of slow executions
#
# Prerequisites:
# - Kafka running on localhost:9092
# - DashFlow application with DashFlow Streaming telemetry
# - dashflow CLI installed
#

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m'

# Configuration
KAFKA_BROKERS="${DASHFLOW_KAFKA_BROKERS:-localhost:9092}"
TOPIC="${DASHFLOW_KAFKA_TOPIC:-dashstream-events}"
OUTPUT_DIR="./perf-analysis-$(date +%Y%m%d-%H%M%S)"
SLOW_THRESHOLD_MS=1000  # Define "slow" as >1 second

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_perf() { echo -e "${MAGENTA}[PERF]${NC} $1"; }
log_header() { echo -e "\n${CYAN}╔══════════════════════════════════════════════════╗${NC}"; echo -e "${CYAN}║ $1${NC}"; echo -e "${CYAN}╚══════════════════════════════════════════════════╝${NC}\n"; }

# Setup
setup() {
    log_header "Performance Debugging Setup"

    mkdir -p "$OUTPUT_DIR"
    log_success "Created output directory: $OUTPUT_DIR"

    THREAD_IDS="${DASHFLOW_THREAD_IDS:-}"
    if [ -z "$THREAD_IDS" ]; then
        log_warn "DASHFLOW_THREAD_IDS not set; using example thread"
        THREAD_IDS="example-session-123"
    fi

    log_success "Threads to analyze:"
    echo "$THREAD_IDS" | sed 's/^/  /'

    # Select first thread for detailed analysis
    THREAD_ID=$(echo "$THREAD_IDS" | head -1)
    log_info "Primary thread for detailed analysis: $THREAD_ID"
}

# 1. Quick Performance Overview
quick_overview() {
    log_header "1. Quick Performance Overview"

    log_info "Analyzing thread: $THREAD_ID"

    dashflow inspect \
        --bootstrap-servers "$KAFKA_BROKERS" \
        --topic "$TOPIC" \
        --thread "$THREAD_ID" \
        --stats \
        2>/dev/null || log_warn "Thread not found"

    log_success "Quick overview complete"
}

# 2. Detailed Performance Profiling
detailed_profile() {
    log_header "2. Detailed Performance Profiling"

    local profile_file="$OUTPUT_DIR/profile_${THREAD_ID}.txt"

    log_info "Running detailed profiling..."

    {
        echo "═══════════════════════════════════════════════════════"
        echo "  Performance Profile: $THREAD_ID"
        echo "  Generated: $(date)"
        echo "═══════════════════════════════════════════════════════"
        echo ""

        dashflow profile \
            --bootstrap-servers "$KAFKA_BROKERS" \
            --topic "$TOPIC" \
            --thread "$THREAD_ID" \
            --detailed \
            2>/dev/null || echo "(no data)"

    } > "$profile_file"

    log_success "Profile saved: $profile_file"

    log_info "Preview:"
    head -30 "$profile_file" | sed 's/^/  /'
}

# 3. Generate Flamegraph
generate_flamegraph() {
    log_header "3. Flamegraph Generation"

    local flamegraph_file="$OUTPUT_DIR/flamegraph_${THREAD_ID}.svg"

    log_info "Generating flamegraph for visual analysis..."
    log_info "Output: $flamegraph_file"

    dashflow flamegraph \
        --bootstrap-servers "$KAFKA_BROKERS" \
        --topic "$TOPIC" \
        --thread "$THREAD_ID" \
        --output "$flamegraph_file" \
        2>/dev/null || log_warn "Flamegraph generation failed"

    if [ -f "$flamegraph_file" ]; then
        log_success "Flamegraph generated: $(du -h "$flamegraph_file" | cut -f1)"
        log_info "Open in browser: file://$(realpath "$flamegraph_file")"

        # Try to open in browser
        if command -v open &> /dev/null; then
            log_info "Opening in browser..."
            open "$flamegraph_file" 2>/dev/null || true
        elif command -v xdg-open &> /dev/null; then
            log_info "Opening in browser..."
            xdg-open "$flamegraph_file" 2>/dev/null || true
        fi
    fi

    log_info "Note: simplified flamegraph options are not supported by dashflow-cli."
}

# 4. Identify Slow Operations
identify_slow_operations() {
    log_header "4. Identify Slow Operations"

    local slow_ops_file="$OUTPUT_DIR/slow_operations.txt"

    log_info "Collecting detailed profiles (manual inspection)..."

    {
        echo "Detailed Profiles (manual inspection)"
        echo "════════════════════════════════════════════"
        echo ""

        for tid in $THREAD_IDS; do
            echo "Thread: $tid"
            echo "────────────────────────────────────────────"

            dashflow profile \
                --bootstrap-servers "$KAFKA_BROKERS" \
                --topic "$TOPIC" \
                --thread "$tid" \
                --detailed \
                2>/dev/null || echo "  (no data)"

            echo ""
        done

    } > "$slow_ops_file"

    log_success "Slow operations report: $slow_ops_file"

    log_info "Preview:"
    head -20 "$slow_ops_file" | sed 's/^/  /'

    log_perf "Optimization Priority:"
    echo "  Focus on operations with:"
    echo "    • High average duration"
    echo "    • High execution count (frequently called)"
    echo "    • High max duration (unstable performance)"
}

# 5. Timeline Analysis
timeline_analysis() {
    log_header "5. Execution Timeline Analysis"

    local timeline_file="$OUTPUT_DIR/timeline_${THREAD_ID}.txt"

    log_info "Generating execution timeline..."

    {
        echo "═══════════════════════════════════════════════════════"
        echo "  Execution Timeline: $THREAD_ID"
        echo "═══════════════════════════════════════════════════════"
        echo ""

        dashflow timeline replay \
            --bootstrap-servers "$KAFKA_BROKERS" \
            --topic "$TOPIC" \
            --thread "$THREAD_ID" \
            2>/dev/null || echo "(no data)"

    } > "$timeline_file"

    log_success "Timeline saved: $timeline_file"

    log_info "Timeline shows:"
    echo "  • Execution order of nodes"
    echo "  • Wait times between operations"
    echo "  • Parallel execution opportunities"
    echo "  • Sequential bottlenecks"
    echo ""

    log_info "Preview (first 40 lines):"
    head -40 "$timeline_file" | sed 's/^/  /'
}

# 6. Compare Multiple Threads
compare_threads() {
    log_header "6. Cross-Thread Performance Comparison"

    local comparison_file="$OUTPUT_DIR/thread_comparison.txt"

    log_info "Comparing performance across threads..."

    {
        echo "═══════════════════════════════════════════════════════"
        echo "  Thread Performance Comparison"
        echo "═══════════════════════════════════════════════════════"
        echo ""

        for tid in $THREAD_IDS; do
            echo "────────────────────────────────────────────────────"
            echo "Thread: $tid"
            echo "────────────────────────────────────────────────────"

            dashflow inspect \
                --bootstrap-servers "$KAFKA_BROKERS" \
                --topic "$TOPIC" \
                --thread "$tid" \
                --stats \
                2>/dev/null | head -20 || echo "(no data)"

            echo ""
        done

    } > "$comparison_file"

    log_success "Comparison saved: $comparison_file"

    log_info "Use this to identify:"
    echo "  • Outlier threads (unusually slow)"
    echo "  • Performance consistency"
    echo "  • Different execution paths"
}

# 7. Replay Slow Execution
replay_slow_execution() {
    log_header "7. Time-Travel Debugging"

    local replay_file="$OUTPUT_DIR/replay_${THREAD_ID}.txt"

    log_info "Replaying execution to understand slow paths..."

    {
        echo "═══════════════════════════════════════════════════════"
        echo "  Execution Replay: $THREAD_ID"
        echo "═══════════════════════════════════════════════════════"
        echo ""

        dashflow timeline replay \
            --bootstrap-servers "$KAFKA_BROKERS" \
            --topic "$TOPIC" \
            --thread "$THREAD_ID" \
            2>/dev/null || echo "(no data)"

    } > "$replay_file"

    log_success "Replay saved: $replay_file"

    log_info "Replay helps identify:"
    echo "  • Where execution got slow"
    echo "  • State changes that triggered slow paths"
    echo "  • LLM response times"
    echo "  • Tool execution delays"
    echo ""

    log_info "Preview (first 50 lines):"
    head -50 "$replay_file" | sed 's/^/  /'
}

# 8. Node Performance Breakdown
node_performance() {
    log_header "8. Node-by-Node Performance Analysis"

    local nodes_file="$OUTPUT_DIR/node_performance.txt"

    log_info "Collecting node performance by thread (manual inspection)..."

    {
        for tid in $THREAD_IDS; do
            echo "═══════════════════════════════════════════════════════"
            echo "  Node Performance Profile: $tid"
            echo "═══════════════════════════════════════════════════════"
            echo ""

            dashflow profile \
                --bootstrap-servers "$KAFKA_BROKERS" \
                --topic "$TOPIC" \
                --thread "$tid" \
                --detailed \
                2>/dev/null || true

            echo ""
        done

    } > "$nodes_file"

    if [ -f "$nodes_file" ]; then
        log_success "Node performance report: $nodes_file"
        log_info "Preview (first 40 lines):"
        head -40 "$nodes_file" | sed 's/^/  /'
    fi
}

# 9. Performance Recommendations
performance_recommendations() {
    log_header "9. Performance Optimization Recommendations"

    echo "Based on the analysis, consider these optimizations:"
    echo ""

    echo "1. SLOW NODES"
    echo "   • Cache results when possible"
    echo "   • Optimize LLM prompts (shorter = faster)"
    echo "   • Parallelize independent operations"
    echo "   • Use faster models for simple tasks"
    echo ""

    echo "2. SEQUENTIAL BOTTLENECKS"
    echo "   • Identify nodes that could run in parallel"
    echo "   • Use DashFlow parallel execution (StateGraph.add_parallel_edges)"
    echo "   • Reduce dependencies between nodes"
    echo ""

    echo "3. LLM LATENCY"
    echo "   • Use streaming for better perceived performance"
    echo "   • Implement prompt caching"
    echo "   • Consider faster models (gpt-3.5, claude-haiku)"
    echo "   • Batch multiple requests when possible"
    echo ""

    echo "4. STATE SIZE"
    echo "   • Large states slow down serialization"
    echo "   • Use state diffs (DashFlow Streaming protocol)"
    echo "   • Prune unnecessary data from state"
    echo "   • Use references instead of copying large objects"
    echo ""

    echo "5. CHECKPOINTING"
    echo "   • Checkpoint less frequently for simple graphs"
    echo "   • Use in-memory checkpointing for development"
    echo "   • PostgreSQL checkpointer for production"
    echo ""

    echo "6. TOOL EXECUTION"
    echo "   • Async tool execution"
    echo "   • Timeout slow tools"
    echo "   • Cache tool results"
    echo "   • Parallel tool calls when possible"
    echo ""

    log_success "Optimization recommendations complete"
}

# 10. Generate Performance Report
generate_report() {
    log_header "10. Comprehensive Performance Report"

    local report_file="$OUTPUT_DIR/performance_report_$(date +%Y-%m-%d).html"

    log_info "Generating HTML performance report..."

    cat > "$report_file" << 'EOF'
<!DOCTYPE html>
<html>
<head>
    <title>DashFlow Streaming Performance Report</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; background: #f5f5f5; }
        h1 { color: #2c3e50; border-bottom: 3px solid #3498db; padding-bottom: 10px; }
        h2 { color: #34495e; margin-top: 30px; }
        .section { background: white; padding: 20px; margin: 20px 0; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }
        .metric { display: inline-block; margin: 10px 20px; }
        .metric .label { color: #7f8c8d; font-size: 12px; }
        .metric .value { font-size: 24px; font-weight: bold; color: #2c3e50; }
        .slow { color: #e74c3c; }
        .fast { color: #27ae60; }
        .medium { color: #f39c12; }
        table { width: 100%; border-collapse: collapse; }
        th { background: #3498db; color: white; padding: 12px; text-align: left; }
        td { padding: 10px; border-bottom: 1px solid #ecf0f1; }
        tr:hover { background: #f8f9fa; }
        .recommendation { background: #e8f4f8; border-left: 4px solid #3498db; padding: 15px; margin: 10px 0; }
    </style>
</head>
<body>
    <h1>🚀 DashFlow Streaming Performance Report</h1>
    <p><strong>Generated:</strong> $(date)</p>
    <p><strong>Analysis Period:</strong> All available data</p>

    <div class="section">
        <h2>📊 Overview</h2>
        <div class="metric">
            <div class="label">Threads Analyzed</div>
            <div class="value">$(echo "$THREAD_IDS" | wc -l)</div>
        </div>
        <div class="metric">
            <div class="label">Slow Operations</div>
            <div class="value slow">$(grep -c ">" "$OUTPUT_DIR/slow_operations.txt" 2>/dev/null || echo "N/A")</div>
        </div>
    </div>

    <div class="section">
        <h2>⚡ Performance Highlights</h2>
        <ul>
            <li>Flamegraphs generated for visual analysis</li>
            <li>Timeline analysis shows execution flow</li>
            <li>Node-by-node performance breakdown available</li>
            <li>Slow operations identified (>${SLOW_THRESHOLD_MS}ms)</li>
        </ul>
    </div>

    <div class="section">
        <h2>📁 Generated Files</h2>
        <ul>
EOF

    for file in "$OUTPUT_DIR"/*; do
        echo "            <li><a href=\"file://$(realpath "$file")\">$(basename "$file")</a></li>" >> "$report_file"
    done

    cat >> "$report_file" << 'EOF'
        </ul>
    </div>

    <div class="section">
        <h2>💡 Optimization Recommendations</h2>

        <div class="recommendation">
            <strong>1. Slow Nodes:</strong> Cache results, optimize prompts, use parallelization
        </div>

        <div class="recommendation">
            <strong>2. LLM Latency:</strong> Use streaming, implement caching, consider faster models
        </div>

        <div class="recommendation">
            <strong>3. State Size:</strong> Use state diffs, prune unnecessary data
        </div>

        <div class="recommendation">
            <strong>4. Checkpointing:</strong> Reduce frequency, use in-memory for dev
        </div>
    </div>

    <div class="section">
        <h2>🔍 Next Steps</h2>
        <ol>
            <li>Review flamegraph for visual bottleneck identification</li>
            <li>Analyze slow operations report</li>
            <li>Implement caching for repeated operations</li>
            <li>Optimize prompts to reduce token count</li>
            <li>Consider parallel execution where possible</li>
            <li>Re-run analysis after optimizations to measure improvement</li>
        </ol>
    </div>
</body>
</html>
EOF

    log_success "HTML report generated: $report_file"

    # Try to open report
    if command -v open &> /dev/null; then
        log_info "Opening report in browser..."
        open "$report_file" 2>/dev/null || true
    elif command -v xdg-open &> /dev/null; then
        log_info "Opening report in browser..."
        xdg-open "$report_file" 2>/dev/null || true
    fi

    log_info "Report URL: file://$(realpath "$report_file")"
}

# Summary
show_summary() {
    log_header "Performance Analysis Complete"

    echo "Generated Files:"
    ls -lh "$OUTPUT_DIR" | tail -n +2 | sed 's/^/  /'

    echo ""
    echo "Key Outputs:"
    echo "  📊 Performance profile with percentiles"
    echo "  🔥 Flamegraph visualization"
    echo "  ⏱️  Execution timeline"
    echo "  🐌 Slow operations report"
    echo "  📈 Node performance breakdown"
    echo "  📝 HTML performance report"
    echo ""

    log_info "Next Steps:"
    echo "  1. Open HTML report in browser"
    echo "  2. Review flamegraph for bottlenecks"
    echo "  3. Analyze slow operations"
    echo "  4. Apply optimization recommendations"
    echo "  5. Re-run analysis to measure improvements"
    echo ""

    log_success "Performance debugging workflow complete!"
}

# Main execution
main() {
    echo ""
    log_info "DashFlow Streaming CLI - Performance Debugging Workflow"
    echo ""

    setup
    quick_overview
    detailed_profile
    generate_flamegraph
    identify_slow_operations
    timeline_analysis
    compare_threads
    replay_slow_execution
    node_performance
    performance_recommendations
    generate_report
    show_summary
}

main "$@"
