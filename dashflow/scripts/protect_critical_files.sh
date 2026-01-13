#!/bin/bash
# Protect critical files from deletion
# Called by pre-commit framework
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

set -euo pipefail

# Define critical files that MUST NOT be deleted
CRITICAL_FILES=(
    "scripts/python/json_to_text.py"
)

# ANSI color codes
RED='\033[0;31m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
NC='\033[0m'

# Check if any critical files are being deleted
deleted_critical_files=()

for file in "${CRITICAL_FILES[@]}"; do
    # Check if file is in the index as deleted
    if git diff --cached --name-status | grep -q "^D.*$file"; then
        deleted_critical_files+=("$file")
    fi

    # Also check if file exists on disk
    if [ ! -f "$file" ]; then
        echo -e "${RED}CRITICAL FILE MISSING: $file${NC}" >&2
        deleted_critical_files+=("$file")
    fi
done

# If any critical files are being deleted, block
if [ ${#deleted_critical_files[@]} -gt 0 ]; then
    echo "" >&2
    echo -e "${RED}${BOLD}╔════════════════════════════════════════════════════════════════╗${NC}" >&2
    echo -e "${RED}${BOLD}║           🚨 CRITICAL FILE DELETION BLOCKED 🚨               ║${NC}" >&2
    echo -e "${RED}${BOLD}╚════════════════════════════════════════════════════════════════╝${NC}" >&2
    echo "" >&2
    echo -e "${RED}The following CRITICAL files cannot be deleted:${NC}" >&2
    echo "" >&2
    for file in "${deleted_critical_files[@]}"; do
        echo -e "  ${RED}✗${NC} ${BOLD}$file${NC}" >&2
    done
    echo "" >&2
    echo -e "${YELLOW}json_to_text.py is REQUIRED for Claude output formatting.${NC}" >&2
    echo -e "${YELLOW}It CANNOT be deleted per explicit user instruction.${NC}" >&2
    echo "" >&2
    exit 1
fi

exit 0
