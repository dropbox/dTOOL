#!/bin/bash
#
# DashFlow CLI - Cost Analysis Workflow
#
# This script demonstrates how to use `dashflow costs` for token cost tracking
# and analysis.
#
# Use Cases:
# - Daily/weekly cost reporting
# - Identify expensive operations
# - Per-tenant billing
# - Cost anomaly detection
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
CYAN='\033[0;36m'
NC='\033[0m'

# Configuration
KAFKA_BROKERS="${DASHFLOW_KAFKA_BROKERS:-localhost:9092}"
TOPIC="${DASHFLOW_KAFKA_TOPIC:-dashstream-events}"
OUTPUT_DIR="./cost-analysis-$(date +%Y%m%d-%H%M%S)"

# Cost model (USD per 1M tokens)
INPUT_COST_PER_MILLION="${DASHFLOW_INPUT_COST_PER_MILLION:-0.25}"
OUTPUT_COST_PER_MILLION="${DASHFLOW_OUTPUT_COST_PER_MILLION:-1.25}"

# Optional: analyze a single thread (provide via app logs)
THREAD_ID="${DASHFLOW_THREAD_ID:-}"

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_header() {
  echo -e "\n${CYAN}╔══════════════════════════════════════════════════╗${NC}"
  echo -e "${CYAN}║ $1${NC}"
  echo -e "${CYAN}╚══════════════════════════════════════════════════╝${NC}\n"
}

setup() {
  log_header "Cost Analysis Setup"

  mkdir -p "$OUTPUT_DIR"
  log_success "Created output directory: $OUTPUT_DIR"

  log_info "Kafka: $KAFKA_BROKERS"
  log_info "Topic: $TOPIC"
  log_info "Pricing (USD/1M tokens): input=$INPUT_COST_PER_MILLION output=$OUTPUT_COST_PER_MILLION"
  [ -n "$THREAD_ID" ] && log_info "Thread filter: $THREAD_ID"
}

overall_summary() {
  log_header "1. Overall Cost Summary"

  local output_file="$OUTPUT_DIR/overall_costs.txt"

  dashflow costs \
    --bootstrap-servers "$KAFKA_BROKERS" \
    --topic "$TOPIC" \
    --input-cost-per-million "$INPUT_COST_PER_MILLION" \
    --output-cost-per-million "$OUTPUT_COST_PER_MILLION" \
    2>/dev/null | tee "$output_file" >/dev/null || log_warn "No cost data available"

  [ -f "$output_file" ] && log_success "Saved: $output_file"
}

cost_by_node() {
  log_header "2. Cost Breakdown by Node"

  local output_file="$OUTPUT_DIR/costs_by_node.txt"

  dashflow costs \
    --bootstrap-servers "$KAFKA_BROKERS" \
    --topic "$TOPIC" \
    --by-node \
    --input-cost-per-million "$INPUT_COST_PER_MILLION" \
    --output-cost-per-million "$OUTPUT_COST_PER_MILLION" \
    2>/dev/null | tee "$output_file" >/dev/null || log_warn "No node cost data available"

  [ -f "$output_file" ] && log_success "Saved: $output_file"
}

cost_by_tenant() {
  log_header "3. Cost Breakdown by Tenant"

  local output_file="$OUTPUT_DIR/costs_by_tenant.txt"

  dashflow costs \
    --bootstrap-servers "$KAFKA_BROKERS" \
    --topic "$TOPIC" \
    --by-tenant \
    --input-cost-per-million "$INPUT_COST_PER_MILLION" \
    --output-cost-per-million "$OUTPUT_COST_PER_MILLION" \
    2>/dev/null | tee "$output_file" >/dev/null || log_warn "No tenant cost data available"

  [ -f "$output_file" ] && log_success "Saved: $output_file"
}

thread_costs() {
  if [ -z "$THREAD_ID" ]; then
    log_warn "DASHFLOW_THREAD_ID not set; skipping thread-specific costs"
    return 0
  fi

  log_header "4. Thread-Specific Costs"

  local output_file="$OUTPUT_DIR/thread_${THREAD_ID}_costs.txt"

  dashflow costs \
    --bootstrap-servers "$KAFKA_BROKERS" \
    --topic "$TOPIC" \
    --thread "$THREAD_ID" \
    --by-node \
    --input-cost-per-million "$INPUT_COST_PER_MILLION" \
    --output-cost-per-million "$OUTPUT_COST_PER_MILLION" \
    2>/dev/null | tee "$output_file" >/dev/null || log_warn "No data for thread: $THREAD_ID"

  [ -f "$output_file" ] && log_success "Saved: $output_file"
}

main() {
  echo ""
  log_info "DashFlow CLI - Cost Analysis Workflow"
  log_info "===================================="
  echo ""

  setup
  overall_summary
  cost_by_node
  cost_by_tenant
  thread_costs

  echo ""
  log_success "Cost analysis complete! Results in: $OUTPUT_DIR"
}

main "$@"
