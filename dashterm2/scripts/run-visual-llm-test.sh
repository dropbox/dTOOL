#!/bin/bash
# =============================================================================
# DashTerm2 Visual Comparison Test Runner
# Uses LLM judges (Claude Opus 4.5 / GPT-4o) to evaluate rendering quality
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log() { echo -e "${BLUE}[VISUAL-LLM]${NC} $1"; }
ok() { echo -e "${GREEN}[OK]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; }

# =============================================================================
# Help
# =============================================================================

show_help() {
    cat << EOF
DashTerm2 Visual Comparison Test with LLM-as-Judge

USAGE:
    $0 [OPTIONS]

OPTIONS:
    --all           Run all test cases (default)
    --test-case X   Run a specific test case
    --opus-only     Use only Claude Opus 4.5
    --gpt-only      Use only GPT-4o
    --list-tests    List available test cases
    --check-deps    Check dependencies only
    --help          Show this help

ENVIRONMENT:
    ANTHROPIC_API_KEY   Required for Claude (set in env or .env file)
    OPENAI_API_KEY      Required for GPT (set in env or .env file)
    ANTHROPIC_MODEL     Override Claude model (default: claude-opus-4-5-20250514)
    OPENAI_MODEL        Override GPT model (default: gpt-5.2)

EXAMPLES:
    $0 --all                              # Run all tests with both LLMs
    $0 --all --opus-only                  # Use only Claude Opus 4.5
    $0 --test-case box_drawing_single     # Test box characters only
    $0 --list-tests                       # Show available tests

OUTPUT:
    Results saved to: visual-test-output/llm-judge/<timestamp>/
    - *_comparison.png : Side-by-side comparison panels
    - *_heatmap.png    : Pixel difference heatmaps
    - results.json     : Structured test results
    - report.md        : Human-readable markdown report
EOF
}

# =============================================================================
# Dependency Check
# =============================================================================

check_dependencies() {
    log "Checking dependencies..."

    local missing=()

    # Python packages
    python3 -c "import PIL" 2>/dev/null || missing+=("Pillow")
    python3 -c "import numpy" 2>/dev/null || missing+=("numpy")
    python3 -c "import anthropic" 2>/dev/null || missing+=("anthropic")
    python3 -c "import openai" 2>/dev/null || missing+=("openai")

    if [ ${#missing[@]} -gt 0 ]; then
        error "Missing Python packages: ${missing[*]}"
        echo ""
        echo "Install with:"
        echo "    pip3 install ${missing[*]}"
        return 1
    fi
    ok "Python packages installed"

    # API Keys
    local has_key=false
    if [ -n "${ANTHROPIC_API_KEY:-}" ]; then
        ok "ANTHROPIC_API_KEY is set"
        has_key=true
    else
        warn "ANTHROPIC_API_KEY not set (Opus 4.5 unavailable)"
    fi

    if [ -n "${OPENAI_API_KEY:-}" ]; then
        ok "OPENAI_API_KEY is set"
        has_key=true
    else
        warn "OPENAI_API_KEY not set (GPT-4o unavailable)"
    fi

    if [ "$has_key" = false ]; then
        error "At least one API key required"
        return 1
    fi

    # DashTerm2 build
    if find ~/Library/Developer/Xcode/DerivedData/DashTerm2-*/Build/Products/Development/DashTerm2.app -maxdepth 0 2>/dev/null | head -1 | grep -q .; then
        ok "DashTerm2 build found"
    else
        error "DashTerm2 build not found"
        echo ""
        echo "Build with:"
        echo "    xcodebuild -project DashTerm2.xcodeproj -scheme DashTerm2 -configuration Development build CODE_SIGNING_ALLOWED=NO"
        return 1
    fi

    # iTerm2
    if [ -d "/Applications/iTerm.app" ]; then
        ok "iTerm2 installed"
    else
        error "iTerm2 not found in /Applications"
        return 1
    fi

    ok "All dependencies satisfied"
    return 0
}

# =============================================================================
# Load Environment
# =============================================================================

load_env() {
    # Try to load .env file if it exists
    local env_files=(
        "$PROJECT_ROOT/.env"
        "$HOME/.env"
        "$HOME/.config/dashterm/.env"
    )

    for env_file in "${env_files[@]}"; do
        if [ -f "$env_file" ]; then
            log "Loading environment from: $env_file"
            # shellcheck source=/dev/null
            set -a
            source "$env_file"
            set +a
            break
        fi
    done
}

# =============================================================================
# Main
# =============================================================================

main() {
    # Handle --help early
    if [[ "${1:-}" == "--help" ]] || [[ "${1:-}" == "-h" ]]; then
        show_help
        exit 0
    fi

    # Load environment variables
    load_env

    # Handle --check-deps
    if [[ "${1:-}" == "--check-deps" ]]; then
        check_dependencies
        exit $?
    fi

    # Check dependencies before running
    if ! check_dependencies; then
        exit 1
    fi

    echo ""
    log "Starting visual comparison test..."
    echo ""

    # Pass all arguments to Python script
    cd "$PROJECT_ROOT"

    if [ $# -eq 0 ]; then
        # Default to --all
        python3 "$SCRIPT_DIR/llm_visual_judge.py" --all
    else
        python3 "$SCRIPT_DIR/llm_visual_judge.py" "$@"
    fi
}

main "$@"
