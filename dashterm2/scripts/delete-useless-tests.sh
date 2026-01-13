#!/bin/bash
# Identify tests that can be safely deleted (they prove nothing)
# These tests ONLY check if a string exists in source code

TEST_FILE="DashTerm2Tests/BugRegressionTests.swift"

echo "=== TESTS THAT CAN BE DELETED ==="
echo "These tests only check if strings exist in source files."
echo "They prove NOTHING about whether bugs are fixed."
echo ""

# Pattern: tests that ONLY do loadSourceFile + content.contains
# No actual instantiation of production classes

echo "Category 1: Branding-only tests"
grep -n 'func test_BUG.*[Bb]randing' "$TEST_FILE" | wc -l

echo ""
echo "Category 2: Tests with ONLY string checks (no class instantiation)"
echo "Scanning for tests without NSClassFromString, without actual class names..."

# Count tests that have loadSourceFile but no production class usage
grep -c 'loadSourceFile' "$TEST_FILE"

echo ""
echo "=== RECOMMENDATION ==="
echo "1. Delete all branding-check tests"
echo "2. Delete tests that only verify string existence"
echo "3. Keep tests that instantiate actual production classes"
echo "4. Focus worker time on 357 high-priority bugs"
