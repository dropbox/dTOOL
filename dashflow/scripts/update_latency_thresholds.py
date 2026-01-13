#!/usr/bin/env python3
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
"""
Update latency thresholds in golden dataset based on measured latencies.

Strategy:
- Set threshold = measured_latency * 1.2 (20% buffer) with minimum 2000ms buffer
- Round to nearest 1000ms for cleaner thresholds
- Never decrease existing thresholds (only increase if needed)
- Minimum threshold: 5000ms
"""

import json
import glob
import os
from pathlib import Path
import math

def calculate_new_threshold(measured_latency_ms: int, current_threshold_ms: int) -> int:
    """
    Calculate new latency threshold based on measured latency.

    Args:
        measured_latency_ms: Actual measured latency from eval
        current_threshold_ms: Current threshold in dataset

    Returns:
        New threshold (rounded to nearest 1000ms)
    """
    # Strategy 1: 20% buffer, minimum 2000ms buffer
    buffer_ms = max(measured_latency_ms * 0.2, 2000)
    new_threshold = measured_latency_ms + buffer_ms

    # Round to nearest 1000ms
    new_threshold = math.ceil(new_threshold / 1000) * 1000

    # Minimum threshold: 5000ms
    new_threshold = max(new_threshold, 5000)

    # Never decrease threshold
    new_threshold = max(new_threshold, current_threshold_ms)

    return int(new_threshold)


def main():
    # Load eval results (updated from document_search to librarian - app consolidation Dec 2025)
    eval_results_path = Path('examples/apps/librarian/outputs/eval_report.json')
    with open(eval_results_path, 'r') as f:
        eval_data = json.load(f)

    # Build map of scenario_id -> measured_latency
    latencies = {}
    for scenario in eval_data['scenarios']:
        latencies[scenario['id']] = scenario['latency_ms']

    print(f"Loaded {len(latencies)} scenario latencies from eval results")

    # Process each golden dataset file
    dataset_dir = Path('examples/apps/librarian/golden_dataset')
    json_files = sorted(glob.glob(str(dataset_dir / '*.json')))

    updated_count = 0
    unchanged_count = 0

    changes = []

    for json_file in json_files:
        with open(json_file, 'r') as f:
            scenario_data = json.load(f)

        scenario_id = scenario_data['id']

        if scenario_id not in latencies:
            print(f"WARNING: No latency data for {scenario_id}, skipping")
            continue

        measured_latency = latencies[scenario_id]
        current_threshold = scenario_data.get('max_latency_ms', 10000)
        new_threshold = calculate_new_threshold(measured_latency, current_threshold)

        if new_threshold != current_threshold:
            scenario_data['max_latency_ms'] = new_threshold

            # Write back
            with open(json_file, 'w') as f:
                json.dump(scenario_data, f, indent=2)
                f.write('\n')  # Add trailing newline

            updated_count += 1
            changes.append({
                'id': scenario_id,
                'measured': measured_latency,
                'old_threshold': current_threshold,
                'new_threshold': new_threshold,
                'increase': new_threshold - current_threshold
            })
        else:
            unchanged_count += 1

    print(f"\n=== Update Summary ===")
    print(f"Total scenarios: {len(json_files)}")
    print(f"Updated: {updated_count}")
    print(f"Unchanged: {unchanged_count}")

    if changes:
        print(f"\n=== Changes (sorted by increase) ===")
        changes.sort(key=lambda x: x['increase'], reverse=True)

        for change in changes:
            print(f"{change['id']:45} {change['old_threshold']:>6}ms → {change['new_threshold']:>6}ms (+{change['increase']:>5}ms) [measured: {change['measured']:>6}ms]")

        # Statistics
        increases = [c['increase'] for c in changes]
        print(f"\n=== Change Statistics ===")
        print(f"Min increase: {min(increases)}ms")
        print(f"Max increase: {max(increases)}ms")
        print(f"Avg increase: {sum(increases) / len(increases):.0f}ms")


if __name__ == '__main__':
    main()
