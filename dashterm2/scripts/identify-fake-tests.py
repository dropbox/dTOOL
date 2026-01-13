#!/usr/bin/env python3
"""
Identify fake tests in BugRegressionTests.swift

A test is considered FAKE if it ONLY does:
- NSClassFromString checks
- instancesRespond(to:) checks
- responds(to:) checks
- loadSourceFile + content.contains

And does NOT actually call methods with real inputs.
"""

import re
import sys

def extract_tests(filepath):
    """Extract all test functions from the file."""
    with open(filepath, 'r') as f:
        content = f.read()

    # Find all test functions
    # Pattern: func test_BUG_XXX_name() { ... }
    test_pattern = r'(func test_BUG_\d+_\w+\(\)[^{]*\{)'

    tests = []
    lines = content.split('\n')

    i = 0
    while i < len(lines):
        line = lines[i]
        if 'func test_BUG_' in line and '()' in line:
            # Found a test function
            test_name_match = re.search(r'func (test_BUG_\d+_\w+)', line)
            if test_name_match:
                test_name = test_name_match.group(1)
                start_line = i

                # Find the closing brace
                brace_count = 0
                test_body = []
                for j in range(i, len(lines)):
                    test_body.append(lines[j])
                    brace_count += lines[j].count('{') - lines[j].count('}')
                    if brace_count == 0 and j > i:
                        break

                tests.append({
                    'name': test_name,
                    'start_line': start_line + 1,  # 1-indexed
                    'end_line': start_line + len(test_body),
                    'body': '\n'.join(test_body)
                })
                i = start_line + len(test_body)
                continue
        i += 1

    return tests

def is_fake_test(test_body):
    """Check if a test is fake (only checks existence, not behavior)."""
    body = test_body.lower()

    # Fake patterns
    fake_patterns = [
        'nsclassfromstring',
        'instancesrespond(to:',
        'responds(to:',
        'loadsourcefile',
        'content.contains',
    ]

    # Real patterns (actually calling methods)
    real_patterns = [
        '.perform(',
        '.invoke(',
        'dispatchqueue.concurrentperform',
        '.startTransfer',
        '.process(',
        '.handle(',
        '.parse(',
        '.decode(',
        '.encode(',
        '.validate(',
        '.execute(',
        '.run(',
        '.send(',
        '.receive(',
        '.read(',
        '.write(',
        '.open(',
        '.close(',
        '.connect(',
        '.disconnect(',
        '.subscribe(',
        '.publish(',
        '.get(',
        '.set(',
        '.add(',
        '.remove(',
        '.insert(',
        '.delete(',
        '.update(',
        '.create(',
        '.destroy(',
        '.init(',
        '.load(',
        '.save(',
        '.fetch(',
        '.commit(',
        '.rollback(',
        '.begin(',
        '.end(',
        '.start(',
        '.stop(',
        '.pause(',
        '.resume(',
        '.cancel(',
        '.reset(',
        '.clear(',
        '.flush(',
        '.sync(',
        '.async(',
    ]

    has_fake = any(p in body for p in fake_patterns)
    has_real = any(p in body for p in real_patterns)

    # If it has fake patterns and no real patterns, it's fake
    if has_fake and not has_real:
        return True

    # If it ONLY has XCTAssertNotNil with NSClassFromString, it's fake
    if 'xctassertnotnil' in body and 'nsclassfromstring' in body:
        # Check if there's anything else meaningful
        if not has_real:
            return True

    # If it ONLY has XCTAssertTrue with instancesRespond, it's fake
    if 'xctasserttrue' in body and ('instancesrespond' in body or 'responds(to:' in body):
        if not has_real:
            return True

    return False

def main():
    filepath = 'DashTerm2Tests/BugRegressionTests.swift'

    print("Analyzing tests...")
    tests = extract_tests(filepath)
    print(f"Found {len(tests)} test functions")

    fake_tests = []
    real_tests = []

    for test in tests:
        if is_fake_test(test['body']):
            fake_tests.append(test)
        else:
            real_tests.append(test)

    print(f"\nFake tests: {len(fake_tests)}")
    print(f"Real tests: {len(real_tests)}")

    # Save lists
    with open('docs/test-audit/fake_tests.txt', 'w') as f:
        for t in fake_tests:
            f.write(f"{t['name']} (lines {t['start_line']}-{t['end_line']})\n")

    with open('docs/test-audit/real_tests.txt', 'w') as f:
        for t in real_tests:
            f.write(f"{t['name']} (lines {t['start_line']}-{t['end_line']})\n")

    print(f"\nSaved to docs/test-audit/fake_tests.txt and docs/test-audit/real_tests.txt")

    # Print some examples
    print("\n--- Sample FAKE tests ---")
    for t in fake_tests[:5]:
        print(f"  {t['name']}")

    print("\n--- Sample REAL tests ---")
    for t in real_tests[:5]:
        print(f"  {t['name']}")

if __name__ == '__main__':
    main()
