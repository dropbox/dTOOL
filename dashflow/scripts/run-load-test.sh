#!/bin/bash
# Load Testing Runner Script
# Automates the process of starting the server and running load tests
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
BASE_URL="${BASE_URL:-http://localhost:8080}"
TEST_SCENARIO="${TEST_SCENARIO:-basic-invoke}"
START_SERVER="${START_SERVER:-true}"
CLEANUP="${CLEANUP:-true}"
SERVER_PID=""
PORT_FORWARD_PID=""

# Function to print colored output
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to cleanup on exit
cleanup() {
    if [ "$CLEANUP" = "true" ]; then
        print_info "Cleaning up..."

        if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
            print_info "Stopping server (PID: $SERVER_PID)..."
            kill "$SERVER_PID" 2>/dev/null || true
            wait "$SERVER_PID" 2>/dev/null || true
        fi

        if [ -n "$PORT_FORWARD_PID" ] && kill -0 "$PORT_FORWARD_PID" 2>/dev/null; then
            print_info "Stopping port forward (PID: $PORT_FORWARD_PID)..."
            kill "$PORT_FORWARD_PID" 2>/dev/null || true
        fi

        print_success "Cleanup complete"
    fi
}

trap cleanup EXIT INT TERM

# Function to check if k6 is installed
check_k6() {
    if ! command -v k6 &> /dev/null; then
        print_error "k6 is not installed"
        print_info "Install k6:"
        print_info "  macOS: brew install k6"
        print_info "  Linux: curl https://github.com/grafana/k6/releases/download/v0.48.0/k6-v0.48.0-linux-amd64.tar.gz -L | tar xvz && sudo mv k6-*/k6 /usr/local/bin/"
        print_info "  Docs: https://k6.io/docs/getting-started/installation/"
        exit 1
    fi
    print_success "k6 found ($(k6 version))"
}

# Function to build the project
build_project() {
    print_info "Building project in release mode..."
    if cargo build --release --example basic_skeleton -p dashflow-langserve; then
        print_success "Build complete"
    else
        print_error "Build failed"
        exit 1
    fi
}

# Function to start the server
start_server() {
    print_info "Starting DashFlow Rust server..."

    # Start server in background
    cargo run --release --example basic_skeleton -p dashflow-langserve > server.log 2>&1 &
    SERVER_PID=$!

    print_info "Server started (PID: $SERVER_PID)"
    print_info "Waiting for server to be ready..."

    # Wait for server to be ready (max 30 seconds)
    for i in {1..30}; do
        if curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/health" 2>/dev/null | grep -q "200"; then
            print_success "Server is ready"
            return 0
        fi
        sleep 1
        echo -n "."
    done

    echo ""
    print_error "Server failed to start within 30 seconds"
    print_info "Server logs:"
    tail -n 20 server.log
    exit 1
}

# Function to check if server is already running
check_server() {
    print_info "Checking if server is already running at $BASE_URL..."

    if curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/health" 2>/dev/null | grep -q "200"; then
        print_success "Server is running"
        return 0
    else
        print_warning "Server is not responding at $BASE_URL"
        return 1
    fi
}

# Function to setup port forward for Kubernetes
setup_k8s_port_forward() {
    print_info "Setting up port forward to Kubernetes..."

    # Check if pods are ready
    if ! kubectl get pods -l app=dashflow-rust &>/dev/null; then
        print_error "No dashflow-rust pods found in Kubernetes"
        print_info "Deploy first: kubectl apply -f k8s/"
        exit 1
    fi

    # Wait for pods to be ready
    print_info "Waiting for pods to be ready..."
    kubectl wait --for=condition=ready pod -l app=dashflow-rust --timeout=300s

    # Start port forward
    kubectl port-forward service/dashflow-rust 8080:80 &
    PORT_FORWARD_PID=$!

    print_info "Port forward started (PID: $PORT_FORWARD_PID)"
    print_info "Waiting for port forward to be ready..."
    sleep 5

    if check_server; then
        print_success "Port forward is ready"
    else
        print_error "Port forward failed"
        exit 1
    fi
}

# Function to run the load test
run_test() {
    local scenario=$1
    local scenario_file="load-tests/k6/scenarios/${scenario}.js"

    if [ ! -f "$scenario_file" ]; then
        print_error "Test scenario not found: $scenario_file"
        print_info "Available scenarios:"
        ls -1 load-tests/k6/scenarios/*.js | xargs -n1 basename | sed 's/.js$//'
        exit 1
    fi

    print_info "Running load test: $scenario"
    print_info "Target: $BASE_URL"
    print_info "Scenario file: $scenario_file"
    echo ""

    # Run k6 test
    if BASE_URL="$BASE_URL" k6 run "$scenario_file"; then
        print_success "Load test completed successfully"
    else
        print_error "Load test failed"
        exit 1
    fi
}

# Function to display usage
usage() {
    echo "Usage: $0 [OPTIONS] [SCENARIO]"
    echo ""
    echo "Run load tests against DashFlow Rust"
    echo ""
    echo "Arguments:"
    echo "  SCENARIO              Test scenario to run (default: basic-invoke)"
    echo ""
    echo "Options:"
    echo "  -h, --help            Show this help message"
    echo "  -u, --url URL         Base URL for testing (default: http://localhost:8080)"
    echo "  -s, --start           Start server before testing (default: true)"
    echo "  -n, --no-start        Don't start server (use existing)"
    echo "  -k, --k8s             Test Kubernetes deployment (sets up port forward)"
    echo "  -c, --no-cleanup      Don't cleanup after test"
    echo ""
    echo "Examples:"
    echo "  $0                                    # Run basic-invoke test (starts server)"
    echo "  $0 -n basic-invoke                    # Run against existing server"
    echo "  $0 -u http://prod.example.com smoke   # Run smoke test against production"
    echo "  $0 -k mixed-workload                  # Test K8s deployment"
    echo "  $0 10m-requests-day                   # Run 10M/day scale test"
    echo ""
    echo "Available scenarios:"
    ls -1 load-tests/k6/scenarios/*.js 2>/dev/null | xargs -n1 basename | sed 's/.js$//' | sed 's/^/  /'
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            usage
            exit 0
            ;;
        -u|--url)
            BASE_URL="$2"
            shift 2
            ;;
        -s|--start)
            START_SERVER="true"
            shift
            ;;
        -n|--no-start)
            START_SERVER="false"
            shift
            ;;
        -k|--k8s)
            K8S_MODE="true"
            shift
            ;;
        -c|--no-cleanup)
            CLEANUP="false"
            shift
            ;;
        *)
            TEST_SCENARIO="$1"
            shift
            ;;
    esac
done

# Main execution
main() {
    print_info "DashFlow Rust Load Testing Runner"
    echo ""

    # Check prerequisites
    check_k6

    # Kubernetes mode
    if [ "$K8S_MODE" = "true" ]; then
        setup_k8s_port_forward
    # Start server mode
    elif [ "$START_SERVER" = "true" ]; then
        if check_server; then
            print_warning "Server already running, skipping start"
        else
            build_project
            start_server
        fi
    # Use existing server
    else
        if ! check_server; then
            print_error "Server is not running at $BASE_URL"
            print_info "Start server first or use -s flag"
            exit 1
        fi
    fi

    # Run the test
    echo ""
    run_test "$TEST_SCENARIO"

    # Report
    echo ""
    print_success "Test run complete!"
    if [ -n "$SERVER_PID" ]; then
        print_info "Server logs: server.log"
    fi
}

main
