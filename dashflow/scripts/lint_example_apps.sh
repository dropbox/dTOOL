#!/bin/bash
# lint_example_apps.sh - Run platform linter on all example apps (Phase 932)
#
# Scans all example apps for potential platform feature reimplementations.
# This script is designed for CI integration.
#
# Usage:
#   ./scripts/lint_example_apps.sh              # Lint all example apps
#   ./scripts/lint_example_apps.sh --strict     # Fail on any warnings
#   ./scripts/lint_example_apps.sh --json       # JSON output for CI parsing
#   ./scripts/lint_example_apps.sh --sarif      # SARIF output for GitHub/IDE
#   ./scripts/lint_example_apps.sh --app NAME   # Lint specific app only
#
# Exit codes:
#   0 - All apps passed (or only info-level findings)
#   1 - Warnings found (non-strict mode)
#   2 - Errors found
#   3 - Build or execution error
#
# CI Integration examples:
#
#   # GitHub Actions
#   - name: Lint example apps
#     run: ./scripts/lint_example_apps.sh --sarif > lint-results.sarif
#   - uses: github/codeql-action/upload-sarif@v2
#     with:
#       sarif_file: lint-results.sarif
#
#   # GitLab CI
#   lint_apps:
#     script:
#       - ./scripts/lint_example_apps.sh --json > lint-results.json
#     artifacts:
#       reports:
#         codequality: lint-results.json

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
EXAMPLES_DIR="$REPO_ROOT/examples/apps"

# Parse options
STRICT=false
FORMAT="text"
SPECIFIC_APP=""
VERBOSE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --strict)
            STRICT=true
            shift
            ;;
        --json)
            FORMAT="json"
            shift
            ;;
        --sarif)
            FORMAT="sarif"
            shift
            ;;
        --app)
            SPECIFIC_APP="$2"
            shift 2
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            exit 3
            ;;
    esac
done

cd "$REPO_ROOT"

# Build CLI if needed
if [[ "$VERBOSE" == "true" ]]; then
    echo "Building dashflow-cli..." >&2
fi
cargo build -p dashflow-cli --release -q 2>/dev/null || cargo build -p dashflow-cli -q

# Find binary
if [[ -f "$REPO_ROOT/target/release/dashflow" ]]; then
    DASHFLOW="$REPO_ROOT/target/release/dashflow"
else
    DASHFLOW="$REPO_ROOT/target/debug/dashflow"
fi

# Track results
TOTAL_APPS=0
APPS_WITH_WARNINGS=0
APPS_WITH_ERRORS=0
ALL_RESULTS=()

# Get list of apps to lint
if [[ -n "$SPECIFIC_APP" ]]; then
    APPS=("$EXAMPLES_DIR/$SPECIFIC_APP")
else
    APPS=()
    for dir in "$EXAMPLES_DIR"/*/; do
        # Skip non-app directories
        if [[ -f "$dir/Cargo.toml" ]]; then
            APPS+=("$dir")
        fi
    done
fi

# JSON/SARIF header for combined output
if [[ "$FORMAT" == "json" ]]; then
    echo '{"lint_results": ['
fi

if [[ "$FORMAT" == "sarif" ]]; then
    echo '{"$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json", "version": "2.1.0", "runs": ['
fi

FIRST_RESULT=true

# Lint each app
for app_dir in "${APPS[@]}"; do
    APP_NAME=$(basename "$app_dir")

    # Skip common utility directory
    if [[ "$APP_NAME" == "common" ]]; then
        continue
    fi

    TOTAL_APPS=$((TOTAL_APPS + 1))

    if [[ "$FORMAT" == "text" ]]; then
        echo "=== Linting: $APP_NAME ===" >&2
    fi

    # Run lint and capture output
    LINT_ARGS="--format $FORMAT"
    if [[ "$STRICT" == "true" ]]; then
        LINT_ARGS="$LINT_ARGS --severity error"
    fi

    RESULT=$("$DASHFLOW" lint $LINT_ARGS "$app_dir" 2>&1) || EXIT_CODE=$?
    EXIT_CODE=${EXIT_CODE:-0}

    # Track results
    if [[ $EXIT_CODE -ne 0 ]]; then
        APPS_WITH_ERRORS=$((APPS_WITH_ERRORS + 1))
    elif echo "$RESULT" | grep -q "warning"; then
        APPS_WITH_WARNINGS=$((APPS_WITH_WARNINGS + 1))
    fi

    # Output based on format
    if [[ "$FORMAT" == "text" ]]; then
        if [[ -n "$RESULT" ]] && ! echo "$RESULT" | grep -q "No potential reimplementations"; then
            echo "$RESULT"
        else
            echo "  No issues found"
        fi
        echo
    elif [[ "$FORMAT" == "json" ]]; then
        if [[ "$FIRST_RESULT" != "true" ]]; then
            echo ","
        fi
        echo "{\"app\": \"$APP_NAME\", \"results\": $RESULT}"
        FIRST_RESULT=false
    elif [[ "$FORMAT" == "sarif" ]]; then
        if [[ "$FIRST_RESULT" != "true" ]]; then
            echo ","
        fi
        # For SARIF, inject app name into the run
        echo "$RESULT" | sed "s/\"automationDetails\":/\"automationDetails\": {\"id\": \"$APP_NAME\"},/"
        FIRST_RESULT=false
    fi
done

# JSON/SARIF footer
if [[ "$FORMAT" == "json" ]]; then
    echo ']}'
fi

if [[ "$FORMAT" == "sarif" ]]; then
    echo ']}'
fi

# Print summary (to stderr so it doesn't pollute JSON output)
if [[ "$FORMAT" == "text" ]]; then
    echo "=== Summary ==="
    echo "Total apps scanned: $TOTAL_APPS"
    echo "Apps with warnings: $APPS_WITH_WARNINGS"
    echo "Apps with errors: $APPS_WITH_ERRORS"
fi

# Determine exit code
if [[ $APPS_WITH_ERRORS -gt 0 ]]; then
    exit 2
elif [[ $APPS_WITH_WARNINGS -gt 0 && "$STRICT" == "true" ]]; then
    exit 1
fi

exit 0
