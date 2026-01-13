#!/bin/bash
# TLA+ TLC Model Checker Runner for DashFlow
# Part of TLA-011: Integration with TLC model checker
#
# Usage:
#   ./run_tlc.sh                    # Run all specs
#   ./run_tlc.sh StateGraph         # Run specific spec
#   ./run_tlc.sh --download         # Download tla2tools.jar
#   ./run_tlc.sh --check            # Check prerequisites only
#
# Requirements:
#   - Java 11+ (check with: java -version)
#   - tla2tools.jar (auto-downloaded if missing)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TLA2TOOLS_JAR="$SCRIPT_DIR/tla2tools.jar"
TLA2TOOLS_URL="https://github.com/tlaplus/tlaplus/releases/download/v1.8.0/tla2tools.jar"
RESULTS_FILE="$SCRIPT_DIR/verification_results.md"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# All specs to verify
SPECS=("StateGraph" "ExecutorScheduler" "DeadlockAnalysis" "CheckpointConsistency" "WALAppendOrdering" "DistributedExecution" "StreamMessageOrdering" "FailureRecovery" "ObservabilityOrdering" "RateLimiterFairness")

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

check_java() {
    # Check if java command exists AND actually works
    # (macOS has a stub at /usr/bin/java that just shows install instructions)
    local java_output
    java_output=$(java -version 2>&1)

    if echo "$java_output" | grep -q "Unable to locate a Java Runtime"; then
        log_error "Java not installed. Please install Java 11+:"
        log_error "  macOS: brew install openjdk@11"
        log_error "  Linux: apt install openjdk-11-jdk"
        return 1
    fi

    if ! command -v java &> /dev/null; then
        log_error "Java not found. Please install Java 11+:"
        log_error "  macOS: brew install openjdk@11"
        log_error "  Linux: apt install openjdk-11-jdk"
        return 1
    fi

    # Extract version number - handle both "1.8.0" and "11.0.1" formats
    local java_version
    java_version=$(echo "$java_output" | head -1 | grep -oE '"[0-9]+(\.[0-9]+)*"' | tr -d '"' | cut -d'.' -f1)

    # Handle version 1.x (Java 8 and earlier reported as 1.8, etc.)
    if [[ "$java_version" == "1" ]]; then
        java_version=$(echo "$java_output" | head -1 | grep -oE '"1\.([0-9]+)' | cut -d'.' -f2)
    fi

    if [[ -n "$java_version" ]] && [[ "$java_version" -lt 11 ]]; then
        log_warn "Java version $java_version detected. TLC works best with Java 11+"
    fi
    log_info "Java found: $(echo "$java_output" | head -1)"
    return 0
}

download_tla2tools() {
    if [[ -f "$TLA2TOOLS_JAR" ]]; then
        log_info "tla2tools.jar already exists at $TLA2TOOLS_JAR"
        return 0
    fi

    log_info "Downloading tla2tools.jar from $TLA2TOOLS_URL..."
    if command -v curl &> /dev/null; then
        curl -L -o "$TLA2TOOLS_JAR" "$TLA2TOOLS_URL"
    elif command -v wget &> /dev/null; then
        wget -O "$TLA2TOOLS_JAR" "$TLA2TOOLS_URL"
    else
        log_error "Neither curl nor wget found. Please download manually:"
        log_error "  $TLA2TOOLS_URL -> $TLA2TOOLS_JAR"
        return 1
    fi

    log_info "Downloaded tla2tools.jar successfully"
}

check_prerequisites() {
    log_info "Checking prerequisites..."
    local all_good=true

    if check_java; then
        echo "  [✓] Java"
    else
        echo "  [✗] Java"
        all_good=false
    fi

    if [[ -f "$TLA2TOOLS_JAR" ]]; then
        echo "  [✓] tla2tools.jar"
    else
        echo "  [✗] tla2tools.jar (run: $0 --download)"
        all_good=false
    fi

    for spec in "${SPECS[@]}"; do
        local tla_file="$SCRIPT_DIR/$spec.tla"
        local mc_file="$SCRIPT_DIR/${spec}MC.tla"
        if [[ -f "$mc_file" ]] && [[ -f "$SCRIPT_DIR/$spec.cfg" ]]; then
            echo "  [✓] ${spec}MC.tla + $spec.cfg"
        elif [[ -f "$tla_file" ]] && [[ -f "$SCRIPT_DIR/$spec.cfg" ]]; then
            echo "  [✓] $spec.tla + $spec.cfg"
        else
            echo "  [✗] $spec (missing .tla/.cfg or *MC.tla)"
            all_good=false
        fi
    done

    if $all_good; then
        log_info "All prerequisites satisfied"
        return 0
    else
        log_warn "Some prerequisites missing"
        return 1
    fi
}

run_tlc() {
    local spec_name="$1"
    local tla_file="$SCRIPT_DIR/$spec_name.tla"
    local mc_file="$SCRIPT_DIR/${spec_name}MC.tla"
    local cfg_file="$SCRIPT_DIR/$spec_name.cfg"
    local output_file="$SCRIPT_DIR/${spec_name}_output.txt"

    if [[ -f "$mc_file" ]]; then
        tla_file="$mc_file"
    elif [[ ! -f "$tla_file" ]]; then
        log_error "TLA+ spec not found: $tla_file (or $mc_file)"
        return 1
    fi

    if [[ ! -f "$cfg_file" ]]; then
        log_error "Config file not found: $cfg_file"
        return 1
    fi

    log_info "Running TLC on $spec_name..."

    # Run TLC model checker
    # -workers auto: Use all available cores
    # -deadlock: Check for deadlocks (in addition to specified properties)
    # -cleanup: Clean up temporary files
    local start_time
    start_time=$(date +%s)

    cd "$SCRIPT_DIR"
    if java -XX:+UseParallelGC -Xmx4g -jar "$TLA2TOOLS_JAR" \
        -config "$spec_name.cfg" \
        -workers auto \
        -cleanup \
        "$(basename "$tla_file")" > "$output_file" 2>&1; then
        local end_time
        end_time=$(date +%s)
        local duration=$((end_time - start_time))

        # Check output for errors
        if grep -q "Error:" "$output_file"; then
            log_error "$spec_name: TLC found errors (${duration}s)"
            cat "$output_file"
            return 1
        elif grep -q "No errors" "$output_file" || grep -q "Model checking completed" "$output_file"; then
            log_info "$spec_name: PASSED (${duration}s)"
            # Extract key statistics
            grep -E "(states generated|distinct states|states|depth)" "$output_file" || true
            return 0
        else
            log_warn "$spec_name: Unknown result (${duration}s)"
            cat "$output_file"
            return 1
        fi
    else
        log_error "$spec_name: TLC execution failed"
        cat "$output_file"
        return 1
    fi
}

generate_results_report() {
    local timestamp
    timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

    cat > "$RESULTS_FILE" << EOF
# TLA+ Verification Results

**Generated:** $timestamp
**Runner:** run_tlc.sh (TLA-011)

## Summary

| Specification | Status | Notes |
|--------------|--------|-------|
EOF

    local all_passed=true
    for spec in "${SPECS[@]}"; do
        local output_file="$SCRIPT_DIR/${spec}_output.txt"
        if [[ -f "$output_file" ]]; then
            if grep -q "No errors" "$output_file" || grep -q "Model checking completed" "$output_file"; then
                local states
                states=$(grep -Eo "[0-9][0-9,]* distinct states" "$output_file" | tail -1 | tr -d ',')
                states=${states:-N/A}
                echo "| $spec | ✅ PASSED | $states |" >> "$RESULTS_FILE"
            else
                echo "| $spec | ❌ FAILED | See ${spec}_output.txt |" >> "$RESULTS_FILE"
                all_passed=false
            fi
        else
            echo "| $spec | ⏳ NOT RUN | Run verification first |" >> "$RESULTS_FILE"
        fi
    done

    cat >> "$RESULTS_FILE" << EOF

## Specifications Verified

### StateGraph.tla (TLA-001)
- Core graph execution state machine
- Properties: TypeInvariant, RecursionLimitRespected, Safety, EventuallyTerminates

### ExecutorScheduler.tla (TLA-002)
- Work-stealing scheduler algorithm
- Properties: NoDoubleAssignment, TaskCountInvariant, AllTasksComplete, NoStarvation

### DeadlockAnalysis.tla (TLA-003)
- Deadlock freedom verification
- Properties: NoDeadlock, SemaphoreNonNegative, EventuallyTerminates, NoLivelock

### CheckpointConsistency.tla (TLA-004)
- FileCheckpointer crash consistency model (atomic rename + index safety)
- Properties: IndexReferencesExistingCheckpoint

## How to Run

\`\`\`bash
cd specs/tla
./run_tlc.sh --check     # Check prerequisites
./run_tlc.sh --download  # Download TLC if needed
./run_tlc.sh             # Run all verifications
./run_tlc.sh StateGraph  # Run single spec
\`\`\`

## Requirements

- Java 11+ (\`brew install openjdk@11\`)
- tla2tools.jar (auto-downloaded by script)
EOF

    log_info "Results written to $RESULTS_FILE"

    if $all_passed; then
        return 0
    else
        return 1
    fi
}

main() {
    case "${1:-}" in
        --download)
            check_java || exit 1
            download_tla2tools
            ;;
        --check)
            check_prerequisites
            ;;
        --help|-h)
            echo "TLA+ TLC Model Checker Runner for DashFlow"
            echo ""
            echo "Usage:"
            echo "  $0                    Run all specs"
            echo "  $0 <spec_name>        Run specific spec (e.g., StateGraph)"
            echo "  $0 --download         Download tla2tools.jar"
            echo "  $0 --check            Check prerequisites only"
            echo "  $0 --help             Show this help"
            echo ""
            echo "Available specs: ${SPECS[*]}"
            ;;
        "")
            # Run all specs
            check_java || exit 1
            download_tla2tools || exit 1

            local failed=0
            for spec in "${SPECS[@]}"; do
                if ! run_tlc "$spec"; then
                    ((failed++))
                fi
            done

            generate_results_report

            if [[ $failed -gt 0 ]]; then
                log_error "$failed of ${#SPECS[@]} specs failed"
                exit 1
            else
                log_info "All ${#SPECS[@]} specs passed"
            fi
            ;;
        *)
            # Run specific spec
            check_java || exit 1
            download_tla2tools || exit 1
            run_tlc "$1"
            ;;
    esac
}

main "$@"
