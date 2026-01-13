#!/usr/bin/env python3
"""
Add XCTFail to all mock tests in BugRegressionTests.swift

A mock test is identified by containing 'class Safe' or 'func safe' patterns
inside the test function body.
"""

import re
import sys

def process_file(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    # Pattern to match test functions
    # We need to find test functions and check if they contain mock patterns
    test_func_pattern = r'(func test_[A-Za-z0-9_]+\([^)]*\)\s*\{)'

    lines = content.split('\n')
    output_lines = []
    i = 0
    modified_count = 0

    while i < len(lines):
        line = lines[i]

        # Check if this is a test function declaration
        if re.search(r'^\s*func test_[A-Za-z0-9_]+\([^)]*\)\s*\{', line):
            # Find the end of this function by counting braces
            func_start = i
            brace_count = line.count('{') - line.count('}')
            func_lines = [line]
            j = i + 1

            while j < len(lines) and brace_count > 0:
                func_lines.append(lines[j])
                brace_count += lines[j].count('{') - lines[j].count('}')
                j += 1

            func_body = '\n'.join(func_lines)

            # Check if this function contains mock patterns
            has_mock = (
                'class Safe' in func_body or
                re.search(r'\bfunc safe[A-Z]', func_body) or
                re.search(r'\bfunc mock[A-Z]', func_body) or
                re.search(r'\bfunc simulate[A-Z]', func_body)
            )

            # Check if XCTFail already exists for mock warning
            already_has_fail = 'MOCK TEST' in func_body

            if has_mock and not already_has_fail:
                # Add XCTFail after the opening brace
                output_lines.append(line)
                # Get the indentation
                indent_match = re.match(r'^(\s*)', line)
                base_indent = indent_match.group(1) if indent_match else ''
                inner_indent = base_indent + '        '  # Add 8 spaces for inside function

                output_lines.append(f'{inner_indent}XCTFail("⛔️ MOCK TEST - This test creates fake classes instead of testing production code. Rewrite to call ACTUAL production class!")')
                modified_count += 1
                i += 1
                continue

        output_lines.append(line)
        i += 1

    with open(filepath, 'w') as f:
        f.write('\n'.join(output_lines))

    return modified_count

if __name__ == '__main__':
    filepath = sys.argv[1] if len(sys.argv) > 1 else 'DashTerm2Tests/BugRegressionTests.swift'
    count = process_file(filepath)
    print(f"Added XCTFail to {count} mock tests")
