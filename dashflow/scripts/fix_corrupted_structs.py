#!/usr/bin/env python3
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
"""
Fix corrupted struct definitions in Rust example files.

The corruption pattern:
1. Struct field ends with just part of field name (e.g., "o" instead of "output: String,")
2. The impl MergeableState block is inserted in the middle
3. The closing brace has the rest: "}utput: String,"

This script reconstructs the proper struct definition.
"""

import re
import sys
from pathlib import Path

def fix_corrupted_struct(content):
    """Fix a corrupted struct by finding the split field and reconstructing it."""

    # Find pattern: struct definition with truncated field, then impl, then }field_rest
    pattern = r'(struct\s+\w+\s*\{[^}]*?)\n\s+([a-z_]+)\n\n(impl\s+MergeableState\s+for\s+\w+\s*\{(?:[^}]|\}(?!utput|eview|essage|tatus|rror|esult))*\})(}[a-z_]+:\s*[^,]+,)\n\}'

    def replacer(match):
        struct_start = match.group(1)
        truncated_field = match.group(2)
        impl_block = match.group(3)
        field_rest = match.group(4)

        # Extract the full field from field_rest
        # field_rest looks like: "}output: String,"
        full_field_part = field_rest[1:]  # Remove leading }
        full_field_name = truncated_field + full_field_part.split(':')[0]
        field_type = full_field_part.split(':', 1)[1].strip()

        # Reconstruct
        result = f"{struct_start}\n    {full_field_name}: {field_type}\n}}\n\n{impl_block}"
        return result

    # Try multiple common field endings
    common_endings = ['utput', 'eview_passed', 'essage', 'tatus', 'rror', 'esult', 'etry_count', 'ode']

    for ending in common_endings:
        # More flexible pattern
        pattern = rf'(struct\s+\w+\s*\{{[^}}]*?)\n\s+([a-z_]+)\n\n(impl\s+MergeableState\s+for\s+\w+\s*\{{.*?\n\s*}})(}}{ending}:\s*[^,]+,)\n}}'
        if re.search(pattern, content, re.DOTALL):
            content = re.sub(pattern, lambda m: reconstruct_struct(m, ending), content, flags=re.DOTALL)
            return content, True

    # Generic pattern for any field
    lines = content.split('\n')
    result = []
    i = 0
    fixed = False

    while i < len(lines):
        line = lines[i]

        # Check if this is a struct with potential corruption
        if 'struct ' in line and '{' in line:
            # Look ahead for truncated field pattern
            struct_lines = [line]
            i += 1
            while i < len(lines) and not ('}' in lines[i] and 'impl' not in lines[i]):
                struct_lines.append(lines[i])
                i += 1

            # Check if last field line is truncated (single word with no : or ,)
            if len(struct_lines) > 1:
                last_field_line = struct_lines[-1].strip()
                if re.match(r'^[a-z_]+$', last_field_line):
                    # Found truncation! Now find the impl block and closing brace
                    truncated_field = last_field_line
                    impl_lines = []

                    # Skip blank line
                    if i < len(lines) and not lines[i].strip():
                        i += 1

                    # Collect impl block
                    impl_start = i
                    brace_count = 0
                    while i < len(lines):
                        impl_lines.append(lines[i])
                        if '{' in lines[i]:
                            brace_count += 1
                        if '}' in lines[i]:
                            brace_count -= 1
                            if brace_count == 0:
                                break
                        i += 1

                    # Check next line for }field_rest pattern
                    i += 1
                    if i < len(lines):
                        next_line = lines[i]
                        if re.match(r'^}[a-z_]+:', next_line):
                            # Found it! Reconstruct
                            field_rest = next_line[1:]  # Remove }
                            full_field = '    ' + truncated_field + field_rest

                            # Write corrected struct
                            result.extend(struct_lines[:-1])  # All but truncated line
                            result.append(full_field)
                            result.append('}')
                            result.append('')
                            result.extend(impl_lines)

                            fixed = True
                            i += 1
                            continue

            # No corruption found, add as-is
            result.extend(struct_lines)
            if i < len(lines):
                result.append(lines[i])
        else:
            result.append(line)

        i += 1

    return '\n'.join(result), fixed

def reconstruct_struct(match, ending):
    """Helper to reconstruct struct from regex match."""
    struct_start = match.group(1)
    truncated_field = match.group(2)
    impl_block = match.group(3)
    field_rest = match.group(4)

    full_field_part = field_rest[1:]  # Remove leading }
    full_field = truncated_field + full_field_part.split(':')[0]
    field_type = full_field_part.split(':', 1)[1].strip()

    return f"{struct_start}\n    {full_field}: {field_type}\n}}\n\n{impl_block}"

def main():
    if len(sys.argv) < 2:
        print("Usage: fix_corrupted_structs.py <file_or_directory>")
        sys.exit(1)

    path = Path(sys.argv[1])

    if path.is_file():
        files = [path]
    elif path.is_dir():
        files = list(path.glob('*.rs'))
    else:
        print(f"Error: {path} is not a file or directory")
        sys.exit(1)

    fixed_count = 0
    for file_path in files:
        with open(file_path, 'r') as f:
            content = f.read()

        new_content, was_fixed = fix_corrupted_struct(content)

        if was_fixed:
            with open(file_path, 'w') as f:
                f.write(new_content)
            print(f"Fixed: {file_path}")
            fixed_count += 1

    print(f"\nTotal files fixed: {fixed_count}")

if __name__ == '__main__':
    main()
