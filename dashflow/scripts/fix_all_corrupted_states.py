#!/usr/bin/env python3
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
"""
Fix ALL corrupted state structs by finding the pattern and reconstructing properly.
"""

import re
from pathlib import Path


def fix_file(file_path: Path) -> bool:
    """Fix corrupted struct in a single file."""
    content = file_path.read_text()
    original = content

    # Pattern: last field is truncated, impl inserted, then rest of field + closing brace
    # Example:
    #   field: Type,
    #   r           <- truncated field name
    #
    # impl MergeableState for State {
    #   ...
    # }esult: String,  <- rest of field name + type + comma
    # }                <- struct closing brace

    # Find all occurrences
    pattern = r'(struct (\w+State) \{[^}]*?)(\w+)\s*\n\s*impl MergeableState for (\w+State) \{(.*?)\}(\w+):\s*([^,]+),\s*\}'

    def fix_match(match):
        struct_start = match.group(1)   # "struct State { ... field: Type,"
        struct_name = match.group(2)    # "State"
        field_start = match.group(3)    # truncated field name start (e.g., "r")
        impl_struct_name = match.group(4)  # State name in impl
        impl_body = match.group(5)      # impl body
        field_end = match.group(6)      # rest of field name (e.g., "esult")
        field_type = match.group(7)     # field type (e.g., "String")

        # Sanity check
        if struct_name != impl_struct_name:
            print(f"  WARNING: struct name mismatch: {struct_name} vs {impl_struct_name}")
            return match.group(0)  # Don't fix if names don't match

        # Reconstruct
        full_field = f"{field_start}{field_end}"
        reconstructed = f"{struct_start}{full_field}: {field_type},\n}}\n\nimpl MergeableState for {struct_name} {{{impl_body}}}"

        return reconstructed

    content = re.sub(pattern, fix_match, content, flags=re.MULTILINE | re.DOTALL)

    if content != original:
        file_path.write_text(content)
        return True

    return False


def main():
    examples_dir = Path("/Users/ayates/dashflow_rs/crates/dashflow-dashflow/examples")

    failing_files = [
        "checkpointing_workflow",
        "conditional_branching",
        "confidence_calibration",
        "crag_agent",
        "distributed_checkpointing",
        "mandatory_tool_context",
        "multi_agent_research",
        "multi_strategy_agent",
        "quality_evaluation_suite",
        "streaming_workflow",
        "traced_agent",
        "v1_0_with_warnings",
        "work_stealing_scheduler",
    ]

    fixed = 0
    for name in failing_files:
        file_path = examples_dir / f"{name}.rs"
        if file_path.exists():
            if fix_file(file_path):
                print(f"✓ Fixed {file_path.name}")
                fixed += 1
        else:
            print(f"✗ File not found: {file_path}")

    print(f"\n✓ Fixed {fixed} files")


if __name__ == "__main__":
    main()
