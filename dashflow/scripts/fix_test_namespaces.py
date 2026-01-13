#!/usr/bin/env python3
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
"""
Script to update InMemoryRecordManager test namespaces from generic "test"
to unique test function names for better test isolation.
"""

import re
import sys
from pathlib import Path


def fix_test_namespaces(file_path: Path) -> tuple[int, list[str]]:
    """
    Fix generic "test" namespaces in a single file.

    Returns:
        (number_of_changes, list_of_changes)
    """
    content = file_path.read_text()
    changes = []
    modified_content = content
    count = 0

    # Find all test functions and their InMemoryRecordManager::new("test") calls
    # Pattern: async fn test_name() { ... InMemoryRecordManager::new("test") ... }

    # Split into lines for easier processing
    lines = modified_content.split('\n')
    current_test_fn = None

    for i, line in enumerate(lines):
        # Check if this is a test function declaration
        test_match = re.match(r'\s*(async\s+)?fn\s+(test_\w+)\s*\(', line)
        if test_match:
            current_test_fn = test_match.group(2)

        # Check if this line has the pattern we want to fix
        if current_test_fn and 'InMemoryRecordManager::new("test")' in line:
            # Replace with the test function name
            old_line = line
            new_line = line.replace(
                'InMemoryRecordManager::new("test")',
                f'InMemoryRecordManager::new("{current_test_fn}")'
            )
            lines[i] = new_line
            count += 1
            changes.append(f"  {current_test_fn}: \"test\" → \"{current_test_fn}\"")

            # Also check if it needs to be split across lines due to length
            # (rustfmt will handle this, but we can prepare for it)
            if len(new_line) > 100:
                # Let rustfmt handle this
                pass

    modified_content = '\n'.join(lines)

    if count > 0:
        file_path.write_text(modified_content)

    return count, changes


def main():
    # Note: These files no longer exist after dashflow-core was merged into dashflow
    files_to_fix = [
        "crates/dashflow/tests/indexing_api_tests.rs",
        "crates/dashflow/src/indexing/record_manager.rs",
    ]

    repo_root = Path(__file__).parent.parent
    total_changes = 0
    all_changes = []

    for file_path_str in files_to_fix:
        file_path = repo_root / file_path_str
        if not file_path.exists():
            print(f"Warning: {file_path} does not exist", file=sys.stderr)
            continue

        print(f"Processing {file_path_str}...")
        count, changes = fix_test_namespaces(file_path)
        total_changes += count
        all_changes.extend(changes)
        print(f"  Fixed {count} occurrences")

    print(f"\nTotal: {total_changes} changes")
    print("\nChanges made:")
    for change in all_changes[:10]:  # Show first 10
        print(change)
    if len(all_changes) > 10:
        print(f"  ... and {len(all_changes) - 10} more")

    return 0 if total_changes > 0 else 1


if __name__ == "__main__":
    sys.exit(main())
