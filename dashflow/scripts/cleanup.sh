#!/bin/bash
# scripts/cleanup.sh - M-83: Clean up local build artifacts
#
# Usage:
#   ./scripts/cleanup.sh           # Dry run - show what would be deleted
#   ./scripts/cleanup.sh --force   # Actually delete the directories
#
# Directories cleaned:
#   - target_*/ (isolated build directories from parallel workers)
#   - fuzz/target/ (fuzzing build artifacts)
#   - .cargo/registry/cache/ (cargo download cache - recoverable)
#
# NOTE: Main target/ is NOT cleaned (preserves incremental compilation).
# Use 'cargo clean' if you want to clean the main target directory.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

FORCE=false
VERBOSE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --force|-f)
            FORCE=true
            shift
            ;;
        --verbose|-v)
            VERBOSE=true
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [--force] [--verbose]"
            echo ""
            echo "Options:"
            echo "  --force, -f    Actually delete directories (default: dry run)"
            echo "  --verbose, -v  Show detailed output"
            echo "  --help, -h     Show this help"
            echo ""
            echo "Cleans up:"
            echo "  - target_*/ directories (parallel worker build artifacts)"
            echo "  - fuzz/target/ (fuzzing artifacts)"
            echo ""
            echo "Does NOT clean:"
            echo "  - target/ (main build directory - use 'cargo clean' for that)"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Function to get directory size in human-readable format
get_size() {
    if [ -d "$1" ]; then
        du -sh "$1" 2>/dev/null | cut -f1
    else
        echo "0B"
    fi
}

# Function to get directory size in bytes (for totaling)
get_size_bytes() {
    if [ -d "$1" ]; then
        du -s "$1" 2>/dev/null | cut -f1
    else
        echo "0"
    fi
}

echo "=== DashFlow Cleanup Script (M-83) ==="
echo ""
echo "Repository: $REPO_ROOT"
echo "Mode: $([ "$FORCE" = true ] && echo "DELETE" || echo "DRY RUN")"
echo ""

# Collect directories to clean
CLEANUP_DIRS=()
TOTAL_BYTES=0

# Pattern: target_*/
for dir in target_*/; do
    if [ -d "$dir" ]; then
        CLEANUP_DIRS+=("$dir")
    fi
done

# fuzz/target/
if [ -d "fuzz/target" ]; then
    CLEANUP_DIRS+=("fuzz/target")
fi

if [ ${#CLEANUP_DIRS[@]} -eq 0 ]; then
    echo "No build artifact directories found to clean."
    echo ""
    echo "Checked for:"
    echo "  - target_*/"
    echo "  - fuzz/target/"
    exit 0
fi

echo "Found ${#CLEANUP_DIRS[@]} directories to clean:"
echo ""

for dir in "${CLEANUP_DIRS[@]}"; do
    size=$(get_size "$dir")
    size_bytes=$(get_size_bytes "$dir")
    TOTAL_BYTES=$((TOTAL_BYTES + size_bytes))
    printf "  %-40s %s\n" "$dir" "$size"
done

echo ""

# Convert total to human-readable (TOTAL_BYTES is in 512-byte blocks from du -s)
# Convert to bytes first (multiply by 512), then to human units
TOTAL_REAL_BYTES=$((TOTAL_BYTES * 512))
if [ $TOTAL_REAL_BYTES -gt 1073741824 ]; then
    TOTAL_HUMAN=$(awk "BEGIN {printf \"%.1fG\", $TOTAL_REAL_BYTES / 1073741824}")
elif [ $TOTAL_REAL_BYTES -gt 1048576 ]; then
    TOTAL_HUMAN=$(awk "BEGIN {printf \"%.1fM\", $TOTAL_REAL_BYTES / 1048576}")
elif [ $TOTAL_REAL_BYTES -gt 1024 ]; then
    TOTAL_HUMAN=$(awk "BEGIN {printf \"%.1fK\", $TOTAL_REAL_BYTES / 1024}")
else
    TOTAL_HUMAN="${TOTAL_REAL_BYTES}B"
fi

echo "Total: ~$TOTAL_HUMAN"
echo ""

if [ "$FORCE" = true ]; then
    echo "Deleting directories..."
    for dir in "${CLEANUP_DIRS[@]}"; do
        if [ "$VERBOSE" = true ]; then
            echo "  rm -rf $dir"
        fi
        rm -rf "$dir"
    done
    echo ""
    echo "Cleanup complete. Freed ~$TOTAL_HUMAN"
else
    echo "This is a DRY RUN. No files were deleted."
    echo ""
    echo "To actually delete these directories, run:"
    echo "  $0 --force"
fi
