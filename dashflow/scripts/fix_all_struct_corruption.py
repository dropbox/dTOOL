#!/usr/bin/env python3
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
"""
Fix corrupted struct definitions where impl block is inserted in the middle.

Pattern:
- Line N: truncated field name (e.g., "    o")
- Lines N+1 to M: impl MergeableState block
- Line M+1: "}field_rest: Type," (e.g., "}utput: String,")
- Line M+2: "}"

Solution:
- Extract full field name from impl block references (other.FIELD)
- Reconstruct struct with complete field definition
- Place impl block after struct closing brace
"""

import re
from pathlib import Path

def fix_file(file_path):
    """Fix a single file's struct corruption."""
    with open(file_path, 'r') as f:
        lines = f.readlines()

    i = 0
    result = []
    fixed = False

    while i < len(lines):
        line = lines[i]

        # Check for struct definition
        if re.match(r'#\[derive.*\]\s*$', line.strip()) and i + 1 < len(lines) and 'struct ' in lines[i+1]:
            # Found a struct, collect it
            result.append(line)
            i += 1
            result.append(lines[i])  # struct StructName {
            i += 1

            struct_fields = []
            truncated_field = None

            # Collect fields until we hit truncation or closing brace
            while i < len(lines):
                field_line = lines[i]
                stripped = field_line.strip()

                # Check if this is a truncated field (single word, no colon)
                if re.match(r'^[a-z_]+$', stripped) and i + 1 < len(lines) and not lines[i+1].strip():
                    truncated_field = stripped
                    i += 1  # skip this line
                    break
                elif stripped == '}':
                    # End of struct, no corruption here
                    result.extend(struct_fields)
                    result.append(field_line)
                    i += 1
                    break
                else:
                    struct_fields.append(field_line)
                    i += 1

            # If we found a truncated field, look for the impl block and completion
            if truncated_field:
                # Skip blank line
                if i < len(lines) and not lines[i].strip():
                    i += 1

                # Collect impl block
                impl_block = []
                if i < len(lines) and 'impl MergeableState' in lines[i]:
                    brace_count = 0
                    while i < len(lines):
                        impl_line = lines[i]
                        impl_block.append(impl_line)
                        brace_count += impl_line.count('{') - impl_line.count('}')
                        i += 1
                        if brace_count == 0 and '}' in impl_line:
                            break

                # Check for completion line: }field_rest: Type,
                if i < len(lines):
                    completion_line = lines[i]
                    match = re.match(r'^}([a-z_]+):\s*(.+)$', completion_line.strip())
                    if match:
                        field_rest = match.group(1)
                        field_type = match.group(2)
                        full_field_name = truncated_field + field_rest

                        # Verify this field is referenced in impl block
                        impl_text = ''.join(impl_block)
                        if f'other.{full_field_name}' in impl_text or f'self.{full_field_name}' in impl_text:
                            # Confirmed! Reconstruct properly
                            result.extend(struct_fields)
                            result.append(f'    {full_field_name}: {field_type}\n')
                            result.append('}\n')
                            result.append('\n')
                            result.extend(impl_block)

                            fixed = True
                            i += 1

                            # Skip the closing } if present
                            if i < len(lines) and lines[i].strip() == '}':
                                i += 1
                            continue

                # If we got here, reconstruction failed - keep original
                result.append(f'    {truncated_field}\n')
                result.extend(impl_block)
                if i < len(lines):
                    result.append(lines[i])
                    i += 1
        else:
            result.append(line)
            i += 1

    if fixed:
        with open(file_path, 'w') as f:
            f.writelines(result)
        return True
    return False

def main():
    examples_dir = Path('crates/dashflow-dashflow/examples')

    # List of known corrupted files
    corrupted_files = [
        'active_learning.rs',
        'batch_processing_pipeline.rs',
        'cascading_agent.rs',
        'code_review_workflow.rs',
        'cost_tracking_example.rs',
        'customer_service_router.rs',
        'dual_path_agent.rs',
        'financial_analysis_agent.rs',
        'graph_events.rs',
        'dashstream_integration.rs',
        'metrics_example.rs',
        'metrics_profiling.rs',
        'multi_strategy_agent.rs',
        'multi_tier_checkpointing.rs',
        'optimized_state_design.rs',
        'parallel_map_reduce.rs',
        'postgres_checkpointing.rs',
        'quality_enforced_agent.rs',
        'sequential_workflow.rs',
        'subgraph_multi_team.rs',
        'template_supervisor.rs',
        'unified_quality_agent.rs',
        'v1_0_legacy_api.rs',
        'v1_0_with_warnings.rs',
    ]

    fixed_count = 0
    for filename in corrupted_files:
        file_path = examples_dir / filename
        if file_path.exists():
            if fix_file(file_path):
                print(f"✅ Fixed: {filename}")
                fixed_count += 1
            else:
                print(f"⚠️  Could not fix: {filename}")
        else:
            print(f"❌ Not found: {filename}")

    print(f"\n📊 Total files fixed: {fixed_count}/{len(corrupted_files)}")

if __name__ == '__main__':
    main()
