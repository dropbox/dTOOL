#!/usr/bin/env python3
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
"""
Add MergeableState implementations to all state structs in examples and tests.
This script adds the import and impl block for any struct ending in 'State'.
"""

import re
import sys
from pathlib import Path


def add_mergeable_state_import(content: str) -> str:
    """Add MergeableState to the imports if not already present."""
    # Check if MergeableState is already imported
    if "MergeableState" in content:
        return content

    # Find dashflow_dashflow imports and add MergeableState
    pattern = r'(use dashflow_dashflow::\{[^}]+)\}'
    match = re.search(pattern, content)

    if match:
        # Add MergeableState to existing import
        old_import = match.group(0)
        if "MergeableState" not in old_import:
            new_import = old_import[:-1] + ", MergeableState}"
            content = content.replace(old_import, new_import)
    else:
        # Try to find simpler import pattern
        pattern = r'use dashflow_dashflow::[^;]+;'
        matches = list(re.finditer(pattern, content))
        if matches:
            # Add after the first dashflow_dashflow import
            last_import = matches[-1].group(0)
            insert_pos = content.find(last_import) + len(last_import)
            content = content[:insert_pos] + "\nuse dashflow_dashflow::MergeableState;" + content[insert_pos:]

    return content


def generate_merge_impl(struct_name: str, fields: list[tuple[str, str]]) -> str:
    """Generate MergeableState impl based on field types."""
    impl_lines = [f"impl MergeableState for {struct_name} {{"]
    impl_lines.append("    fn merge(&mut self, other: Self) {")

    for field_name, field_type in fields:
        # Vec<T> - extend
        if field_type.startswith("Vec<"):
            impl_lines.append(f"        self.{field_name}.extend(other.{field_name});")

        # Option<T> - take other if present
        elif field_type.startswith("Option<"):
            impl_lines.append(f"        if other.{field_name}.is_some() {{")
            impl_lines.append(f"            self.{field_name} = other.{field_name};")
            impl_lines.append("        }")

        # HashMap - extend
        elif field_type.startswith("HashMap<"):
            impl_lines.append(f"        self.{field_name}.extend(other.{field_name});")

        # String - concatenate if non-empty
        elif field_type == "String":
            impl_lines.append(f"        if !other.{field_name}.is_empty() {{")
            impl_lines.append(f"            if self.{field_name}.is_empty() {{")
            impl_lines.append(f"                self.{field_name} = other.{field_name};")
            impl_lines.append("            } else {")
            impl_lines.append(f'                self.{field_name}.push_str("\\n");')
            impl_lines.append(f"                self.{field_name}.push_str(&other.{field_name});")
            impl_lines.append("            }")
            impl_lines.append("        }")

        # bool - OR
        elif field_type == "bool":
            impl_lines.append(f"        self.{field_name} = self.{field_name} || other.{field_name};")

        # Numeric types - max
        elif field_type in ["usize", "u32", "u64", "i32", "i64", "f32", "f64"]:
            if field_type.startswith("f"):
                impl_lines.append(f"        self.{field_name} = self.{field_name}.max(other.{field_name});")
            else:
                impl_lines.append(f"        self.{field_name} = self.{field_name}.max(other.{field_name});")

        # Arc<T> - replace
        elif field_type.startswith("Arc<"):
            impl_lines.append(f"        self.{field_name} = other.{field_name};")

        # Default: replace
        else:
            impl_lines.append(f"        // Replace strategy for {field_name}: {field_type}")
            impl_lines.append(f"        self.{field_name} = other.{field_name};")

    impl_lines.append("    }")
    impl_lines.append("}")

    return "\n".join(impl_lines)


def extract_struct_fields(struct_def: str) -> list[tuple[str, str]]:
    """Extract field names and types from struct definition."""
    fields = []

    # Match field definitions: field_name: Type,
    pattern = r'(\w+):\s*([^,]+),'
    matches = re.findall(pattern, struct_def)

    for field_name, field_type in matches:
        # Clean up type (remove whitespace)
        field_type = field_type.strip()
        fields.append((field_name, field_type))

    return fields


def process_file(file_path: Path) -> bool:
    """Process a single file and add MergeableState implementations."""
    content = file_path.read_text()
    original_content = content

    # Find all state structs
    pattern = r'#\[derive\([^\]]+\)\]\s*struct\s+(\w+State)\s*\{([^}]+)\}'
    matches = list(re.finditer(pattern, content, re.MULTILINE | re.DOTALL))

    if not matches:
        return False

    # Add import first
    content = add_mergeable_state_import(content)

    # Track if we made changes
    changes_made = False

    for match in matches:
        struct_name = match.group(1)
        struct_body = match.group(2)

        # Check if MergeableState impl already exists
        if f"impl MergeableState for {struct_name}" in content:
            continue

        # Extract fields
        fields = extract_struct_fields(struct_body)

        if not fields:
            continue

        # Generate impl
        impl_code = generate_merge_impl(struct_name, fields)

        # Insert impl after struct definition
        struct_end = match.end()
        content = content[:struct_end] + "\n\n" + impl_code + content[struct_end:]

        changes_made = True
        print(f"  Added MergeableState impl for {struct_name}")

    # Write back if changes were made
    if content != original_content:
        file_path.write_text(content)
        return True

    return False


def main():
    # Process examples
    examples_dir = Path("/Users/ayates/dashflow_rs/crates/dashflow-dashflow/examples")
    tests_dir = Path("/Users/ayates/dashflow_rs/crates/dashflow-dashflow/tests")

    total_files = 0
    changed_files = 0

    print("Processing examples...")
    for file_path in examples_dir.glob("*.rs"):
        if process_file(file_path):
            changed_files += 1
            print(f"✓ {file_path.name}")
        total_files += 1

    print("\nProcessing tests...")
    for file_path in tests_dir.glob("*.rs"):
        if process_file(file_path):
            changed_files += 1
            print(f"✓ {file_path.name}")
        total_files += 1

    print(f"\n✓ Processed {total_files} files, modified {changed_files} files")


if __name__ == "__main__":
    main()
