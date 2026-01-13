#!/usr/bin/env bash
# Node.js Tooling Doctor - Checks that Node/npm versions meet requirements
# Usage: ./scripts/check_node_tooling.sh

set -euo pipefail

MIN_NODE_MAJOR=20
MIN_NPM_MAJOR=10
ERRORS=0

echo "=== DashFlow Node.js Tooling Check ==="
echo ""

# Check Node.js
if ! command -v node &> /dev/null; then
    echo "ERROR: Node.js is not installed"
    echo "  Install via: brew install node, or use nvm (see .nvmrc)"
    ERRORS=$((ERRORS + 1))
else
    NODE_VERSION=$(node --version | sed 's/^v//')
    NODE_MAJOR=$(echo "$NODE_VERSION" | cut -d. -f1)
    if [ "$NODE_MAJOR" -lt "$MIN_NODE_MAJOR" ]; then
        echo "ERROR: Node.js $NODE_VERSION is too old (need >= $MIN_NODE_MAJOR.x)"
        echo "  Run: nvm install (uses .nvmrc)"
        ERRORS=$((ERRORS + 1))
    else
        echo "OK: Node.js $NODE_VERSION (>= $MIN_NODE_MAJOR.x required)"
    fi
fi

# Check npm
if ! command -v npm &> /dev/null; then
    echo "ERROR: npm is not installed"
    ERRORS=$((ERRORS + 1))
else
    NPM_VERSION=$(npm --version)
    NPM_MAJOR=$(echo "$NPM_VERSION" | cut -d. -f1)
    if [ "$NPM_MAJOR" -lt "$MIN_NPM_MAJOR" ]; then
        echo "ERROR: npm $NPM_VERSION is too old (need >= $MIN_NPM_MAJOR.x)"
        echo "  Run: npm install -g npm@latest"
        ERRORS=$((ERRORS + 1))
    else
        echo "OK: npm $NPM_VERSION (>= $MIN_NPM_MAJOR.x required)"
    fi
fi

echo ""

# Check .nvmrc
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [ -f "$REPO_ROOT/.nvmrc" ]; then
    NVMRC_VERSION=$(cat "$REPO_ROOT/.nvmrc" | tr -d '[:space:]')
    echo "OK: .nvmrc specifies Node $NVMRC_VERSION"
else
    echo "WARNING: No .nvmrc file found"
fi

echo ""

# Check package.json locations
echo "=== JavaScript Projects ==="
echo ""
echo "1. dashflow-tests (root package.json)"
echo "   Purpose: Test utilities for observability stack"
echo "   Scripts: npm run test:dashboard, test:grafana, test:visual"
echo ""
echo "2. observability-ui/"
echo "   Purpose: React-based observability dashboard UI"
echo "   Scripts: npm run dev, build, test"
echo ""

# Check dependencies are installed
if [ -d "$REPO_ROOT/node_modules" ]; then
    echo "OK: Root node_modules exists"
else
    echo "WARNING: Root node_modules missing - run 'npm install'"
fi

if [ -d "$REPO_ROOT/observability-ui/node_modules" ]; then
    echo "OK: observability-ui/node_modules exists"
else
    echo "WARNING: observability-ui/node_modules missing - run 'cd observability-ui && npm install'"
fi

echo ""

if [ "$ERRORS" -gt 0 ]; then
    echo "=== FAILED: $ERRORS issue(s) found ==="
    exit 1
else
    echo "=== All checks passed ==="
    exit 0
fi
