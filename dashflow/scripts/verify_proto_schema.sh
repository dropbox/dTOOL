#!/bin/bash
# Proto Schema Verification Script
# Regenerates UI proto schema and verifies it matches the committed version.
#
# Usage:
#   ./scripts/verify_proto_schema.sh        # Check if schema is in sync
#   ./scripts/verify_proto_schema.sh --fix  # Regenerate and update the schema
#
# Exit codes:
#   0 - Schema is in sync (or --fix succeeded)
#   1 - Schema is out of sync (run with --fix)
#   2 - Missing dependencies (npm/pbjs not available)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
UI_DIR="$REPO_ROOT/observability-ui"
PROTO_FILE="$REPO_ROOT/proto/dashstream.proto"
SCHEMA_FILE="$UI_DIR/src/proto/dashstream.schema.json"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "=== Proto Schema Verification ==="
echo "Proto source: $PROTO_FILE"
echo "Schema file:  $SCHEMA_FILE"
echo ""

# Check prerequisites
if ! command -v npm &> /dev/null; then
    echo -e "${RED}ERROR: npm not found. Install Node.js/npm first.${NC}"
    exit 2
fi

if [ ! -f "$PROTO_FILE" ]; then
    echo -e "${RED}ERROR: Proto source file not found: $PROTO_FILE${NC}"
    exit 2
fi

if [ ! -d "$UI_DIR/node_modules" ]; then
    echo -e "${YELLOW}Installing UI dependencies...${NC}"
    (cd "$UI_DIR" && npm ci --silent)
fi

# Check if pbjs is available
if [ ! -f "$UI_DIR/node_modules/.bin/pbjs" ]; then
    echo -e "${RED}ERROR: pbjs not found. Run 'npm ci' in observability-ui/${NC}"
    exit 2
fi

# Handle --fix mode
if [ "${1:-}" = "--fix" ]; then
    echo "Regenerating schema..."
    (cd "$UI_DIR" && npm run proto:gen --silent)
    echo -e "${GREEN}Schema regenerated successfully.${NC}"
    exit 0
fi

# Verification mode (default)
echo "Checking schema sync..."

# Generate to temp file and compare
TEMP_SCHEMA=$(mktemp)
trap "rm -f $TEMP_SCHEMA" EXIT

(cd "$UI_DIR" && ./node_modules/.bin/pbjs -t json "$PROTO_FILE" > "$TEMP_SCHEMA" 2>/dev/null)

if diff -q "$TEMP_SCHEMA" "$SCHEMA_FILE" > /dev/null 2>&1; then
    echo -e "${GREEN}Proto schema is in sync.${NC}"
    exit 0
else
    echo -e "${RED}ERROR: Proto schema is out of sync!${NC}"
    echo ""
    echo "The committed schema doesn't match the proto source."
    echo "This can happen when proto/dashstream.proto is modified"
    echo "without regenerating observability-ui/src/proto/dashstream.schema.json"
    echo ""
    echo "To fix, run one of:"
    echo "  ./scripts/verify_proto_schema.sh --fix"
    echo "  cd observability-ui && npm run proto:gen"
    echo ""
    echo "Diff (first 20 lines):"
    diff "$TEMP_SCHEMA" "$SCHEMA_FILE" | head -20 || true
    exit 1
fi
