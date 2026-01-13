#!/usr/bin/env bash
# Phase 910: Check for duplicate worker iteration numbers
#
# Usage:
#   ./scripts/check_iteration_numbers.sh           # Check for duplicates in recent history
#   ./scripts/check_iteration_numbers.sh 1006      # Verify 1006 is safe to use
#   ./scripts/check_iteration_numbers.sh --next    # Show next iteration number
#
# Exit codes:
#   0 - Success (no duplicates found, or number is safe)
#   1 - Error (duplicates found, or number already used)

set -euo pipefail

# Extract all iteration numbers from recent git history
get_used_iterations() {
    git log --oneline -100 | \
        grep -oE "^[a-f0-9]+ # ([0-9]+):" | \
        sed 's/^[a-f0-9]* # \([0-9]*\):.*/\1/' | \
        sort -n
}

# Find the highest iteration number
get_max_iteration() {
    get_used_iterations | tail -1
}

# Find duplicate iteration numbers
find_duplicates() {
    get_used_iterations | uniq -d
}

# Check if a specific number is already used
is_used() {
    local num="$1"
    get_used_iterations | grep -q "^${num}$"
}

# Main logic
case "${1:-check}" in
    --next|next)
        max=$(get_max_iteration)
        if [[ -z "$max" ]]; then
            echo "0"
        else
            echo "$((max + 1))"
        fi
        ;;

    --duplicates|duplicates|check)
        dupes=$(find_duplicates)
        if [[ -n "$dupes" ]]; then
            echo "WARNING: Duplicate iteration numbers found in history:"
            for d in $dupes; do
                echo "  # $d appears multiple times:"
                git log --oneline -100 | grep "# $d:" | sed 's/^/    /'
            done
            exit 1
        else
            echo "OK: No duplicate iteration numbers in recent history"
            exit 0
        fi
        ;;

    --help|-h|help)
        echo "Usage: $0 [check|--next|NUMBER]"
        echo ""
        echo "Commands:"
        echo "  check       Check for duplicate iterations (default)"
        echo "  --next      Show next safe iteration number"
        echo "  NUMBER      Check if NUMBER is safe to use"
        echo ""
        echo "Examples:"
        echo "  $0              # Check for duplicates"
        echo "  $0 --next       # Show next: 1006"
        echo "  $0 1006         # Verify 1006 is available"
        ;;

    *)
        # Assume it's a number to validate
        if [[ "$1" =~ ^[0-9]+$ ]]; then
            if is_used "$1"; then
                echo "ERROR: Iteration # $1 is already used:"
                git log --oneline -100 | grep "# $1:" | sed 's/^/  /'
                echo ""
                echo "Next available: $(($(get_max_iteration) + 1))"
                exit 1
            else
                max=$(get_max_iteration)
                if [[ $1 -le $max ]]; then
                    echo "WARNING: # $1 is less than max ($max) - may cause confusion"
                    exit 1
                fi
                echo "OK: Iteration # $1 is available"
                exit 0
            fi
        else
            echo "Unknown command: $1"
            echo "Run '$0 --help' for usage"
            exit 1
        fi
        ;;
esac
