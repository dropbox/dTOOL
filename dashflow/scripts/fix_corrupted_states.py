#!/usr/bin/env python3
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
"""
Fix corrupted state structs where the Python script inserted impl blocks incorrectly.
"""

import re
from pathlib import Path


def fix_corrupted_struct(content: str) -> tuple[str, bool]:
    """Fix structs where impl was inserted in the middle."""
    changed = False

    # Pattern: field_name (incomplete) followed by impl MergeableState
    # and ending with }...rest_of_field: Type,}
    pattern = r'(\w+):\s*(\w+)\s*\n\s*impl MergeableState for (\w+State)\s*\{(.*?)\}([^}]*):([^,}]+),\s*\}'

    def fix_match(match):
        nonlocal changed
        changed = True

        field_start = match.group(1)  # field name
        type_start = match.group(2)   # start of type
        struct_name = match.group(3)  # State struct name
        impl_body = match.group(4)    # impl body
        type_end = match.group(5)     # rest of field name
        field_type = match.group(6)   # rest of type

        # Reconstruct: field: complete_type,\n}\n\nimpl block
        full_field = f"{field_start}: {type_start}{type_end}: {field_type},"
        full_impl = f"impl MergeableState for {struct_name} {{{impl_body}}}"

        return f"{full_field}\n}}\n\n{full_impl}"

    content = re.sub(pattern, fix_match, content, flags=re.MULTILINE | re.DOTALL)

    # Pattern 2: incomplete field followed by impl
    pattern2 = r',\s*(\w+)\s*\n\s*impl MergeableState'
    if re.search(pattern2, content):
        # More complex - need to find the complete field definition
        # This happens when the last field is truncated
        changed = True
        # Find struct definitions that are incomplete
        content = re.sub(
            r'(struct \w+State \{.*?),\s*(\w+)\s*\n\s*(impl MergeableState for (\w+State))',
            lambda m: f"{m.group(1)},\n    // FIXME: incomplete field: {m.group(2)}\n}}\n\n{m.group(3)}",
            content,
            flags=re.MULTILINE | re.DOTALL
        )

    return content, changed


def main():
    examples_dir = Path("/Users/ayates/dashflow_rs/crates/dashflow-dashflow/examples")
    tests_dir = Path("/Users/ayates/dashflow_rs/crates/dashflow-dashflow/tests")

    total_fixed = 0

    print("Fixing corrupted struct definitions...")

    for directory in [examples_dir, tests_dir]:
        for file_path in directory.glob("*.rs"):
            content = file_path.read_text()
            fixed_content, changed = fix_corrupted_struct(content)

            if changed:
                file_path.write_text(fixed_content)
                total_fixed += 1
                print(f"✓ Fixed {file_path.name}")

    print(f"\n✓ Fixed {total_fixed} files")


if __name__ == "__main__":
    main()
