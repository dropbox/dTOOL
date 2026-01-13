#!/bin/bash
# Triage bugs by category for parallel worker assignment

TEST_FILE="DashTerm2Tests/BugRegressionTests.swift"

echo "=== BUG TRIAGE REPORT ==="
echo ""

echo "🔴 CRASH BUGS (highest priority):"
grep -n 'func test_BUG.*[Cc]rash\|[Nn]il\|[Ff]orce' "$TEST_FILE" | grep -c 'func test'
echo ""

echo "🟠 RACE CONDITIONS:"
grep -n 'func test_BUG.*[Rr]ace\|[Tt]hread\|[Cc]oncurrent' "$TEST_FILE" | grep -c 'func test'
echo ""

echo "🟡 MEMORY BUGS:"
grep -n 'func test_BUG.*[Ll]eak\|[Mm]emory\|[Rr]etain' "$TEST_FILE" | grep -c 'func test'
echo ""

echo "📋 FAKE TESTS (loadSourceFile):"
grep -c 'loadSourceFile' "$TEST_FILE"
echo ""

echo "📋 FAKE TESTS (content.contains only):"
grep -c 'content\.contains' "$TEST_FILE"
echo ""

echo "=== ACTIONABLE ITEMS ==="
echo ""
echo "Tests to DELETE (only check string exists):"
grep -B3 'XCTAssertTrue(content.contains' "$TEST_FILE" | grep 'func test_' | head -20
