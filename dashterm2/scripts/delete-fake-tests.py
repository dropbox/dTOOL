#!/usr/bin/env python3
"""
Delete fake tests from BugRegressionTests.swift

A test is FAKE if it:
1. Uses loadSourceFile() and ONLY does content.contains()
2. Uses NSClassFromString() and ONLY asserts not nil
3. Uses instancesRespond(to:) / responds(to:) and nothing else

Preserves test names in a separate file for later real implementation.
"""

import re
import sys

def main():
    filepath = 'DashTerm2Tests/BugRegressionTests.swift'

    with open(filepath, 'r') as f:
        content = f.read()

    lines = content.split('\n')

    # Fake patterns (if test ONLY has these, delete it)
    fake_only_patterns = [
        r'loadSourceFile\s*\(',
        r'XCTAssertNotNil\s*\(\s*NSClassFromString',
        r'\.instancesRespond\s*\(\s*to:',
        r'\.responds\s*\(\s*to:',
        r'content\.contains\s*\(',
    ]

    # Real patterns (if test has ANY of these, keep it)
    real_patterns = [
        r'\.perform\s*\(',
        r'\.startTransfer',
        r'\.process\s*\(',
        r'\.handle\s*\(',
        r'\.parse\s*\(',
        r'\.decode\s*\(',
        r'\.encode\s*\(',
        r'\.validate\s*\(',
        r'\.execute\s*\(',
        r'DispatchQueue\.concurrentPerform',
        r'autoreleasepool',
        r'expectation\s*\(',
        r'waitForExpectations',
        r'XCTAssertEqual\s*\([^N]',  # XCTAssertEqual with actual values, not NotNil
        r'XCTAssertTrue\s*\([^c]',   # XCTAssertTrue with actual condition, not contains
        r'XCTAssertFalse',
        r'XCTAssertThrows',
        r'XCTAssertNoThrow',
        r'XCTAssertGreater',
        r'XCTAssertLess',
    ]

    # Track test functions
    tests_to_delete = []
    tests_to_keep = []

    i = 0
    while i < len(lines):
        line = lines[i]

        # Find test function start
        match = re.search(r'func\s+(test_BUG_\d+_\w+)\s*\(\s*\)', line)
        if match:
            test_name = match.group(1)
            start_idx = i

            # Find the function body (count braces)
            brace_count = 0
            end_idx = i
            for j in range(i, len(lines)):
                brace_count += lines[j].count('{') - lines[j].count('}')
                if brace_count == 0 and j > i:
                    end_idx = j
                    break

            # Get the function body
            body = '\n'.join(lines[start_idx:end_idx+1])

            # Check if it has real patterns
            has_real = any(re.search(p, body) for p in real_patterns)

            # Check if it has fake patterns
            has_fake = any(re.search(p, body) for p in fake_only_patterns)

            if has_fake and not has_real:
                tests_to_delete.append({
                    'name': test_name,
                    'start': start_idx,
                    'end': end_idx,
                    'body': body
                })
            else:
                tests_to_keep.append({
                    'name': test_name,
                    'start': start_idx,
                    'end': end_idx
                })

            i = end_idx + 1
            continue
        i += 1

    print(f"Tests to DELETE: {len(tests_to_delete)}")
    print(f"Tests to KEEP: {len(tests_to_keep)}")

    # Save deleted test names for reference
    with open('docs/test-audit/deleted_fake_tests.txt', 'w') as f:
        f.write("# Deleted fake tests - these need REAL implementations\n")
        f.write("# Format: test_name\n\n")
        for t in tests_to_delete:
            f.write(f"{t['name']}\n")

    # Create new file content without fake tests
    # Mark lines to delete
    lines_to_delete = set()
    for t in tests_to_delete:
        for line_num in range(t['start'], t['end'] + 1):
            lines_to_delete.add(line_num)

        # Also delete preceding comment block (/// comments)
        comment_start = t['start'] - 1
        while comment_start >= 0 and (lines[comment_start].strip().startswith('///') or lines[comment_start].strip() == ''):
            lines_to_delete.add(comment_start)
            comment_start -= 1

    # Build new content
    new_lines = []
    for idx, line in enumerate(lines):
        if idx not in lines_to_delete:
            new_lines.append(line)

    # Remove excessive blank lines (more than 2 consecutive)
    cleaned_lines = []
    blank_count = 0
    for line in new_lines:
        if line.strip() == '':
            blank_count += 1
            if blank_count <= 2:
                cleaned_lines.append(line)
        else:
            blank_count = 0
            cleaned_lines.append(line)

    # Write new file
    with open(filepath, 'w') as f:
        f.write('\n'.join(cleaned_lines))

    print(f"\nDeleted {len(tests_to_delete)} fake tests")
    print(f"Saved deleted test names to docs/test-audit/deleted_fake_tests.txt")
    print(f"File updated: {filepath}")

    # Show sample of deleted tests
    print("\n--- Sample deleted tests ---")
    for t in tests_to_delete[:10]:
        print(f"  {t['name']}")

if __name__ == '__main__':
    main()
