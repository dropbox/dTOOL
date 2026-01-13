#!/usr/bin/env python3
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
"""
Remove unused dependencies from Cargo.toml files based on cargo-machete output.
Only removes dev-dependencies that are clearly unused test utilities.
"""

import re
import subprocess
import sys
from pathlib import Path

# Safe to remove: test-only dependencies
SAFE_TEST_DEPS = {
    'tokio-test',
    'insta',
    'mockall',
    'wiremock',
}

def get_unused_deps():
    """Run cargo machete and parse output."""
    result = subprocess.run(
        ['cargo', 'machete', '--with-metadata'],
        capture_output=True,
        text=True
    )

    deps_by_crate = {}
    current_crate = None
    current_file = None

    for line in result.stdout.split('\n'):
        # Match crate lines like "dashflow-webscrape -- ./crates/dashflow-webscrape/Cargo.toml:"
        crate_match = re.match(r'^(\S+) -- (.*Cargo\.toml):', line)
        if crate_match:
            current_crate = crate_match.group(1)
            current_file = crate_match.group(2)
            deps_by_crate[current_crate] = {
                'file': current_file,
                'deps': []
            }
            continue

        # Match dependency lines (indented)
        if current_crate and line.startswith('\t'):
            dep = line.strip()
            if dep:
                deps_by_crate[current_crate]['deps'].append(dep)

    return deps_by_crate

def remove_dep_from_toml(toml_path, dep_name):
    """Remove a dependency from a Cargo.toml file."""
    toml_file = Path(toml_path)
    if not toml_file.exists():
        return False

    content = toml_file.read_text()
    lines = content.split('\n')

    new_lines = []
    i = 0
    while i < len(lines):
        line = lines[i]

        # Check if this line declares the dependency
        # Match patterns like: dep_name = "version" or dep_name.workspace = true
        if re.match(rf'^{re.escape(dep_name)}\s*=', line):
            # Skip this line (remove the dependency)
            i += 1
            continue

        new_lines.append(line)
        i += 1

    toml_file.write_text('\n'.join(new_lines))
    return True

def main():
    print("Finding unused dependencies...")
    deps_by_crate = get_unused_deps()

    removed_count = 0
    for crate_name, info in deps_by_crate.items():
        toml_path = info['file']
        for dep in info['deps']:
            if dep in SAFE_TEST_DEPS:
                print(f"Removing {dep} from {crate_name}")
                if remove_dep_from_toml(toml_path, dep):
                    removed_count += 1

    print(f"\nRemoved {removed_count} unused test dependencies")

    # Verify the build still works
    print("\nVerifying build...")
    result = subprocess.run(['cargo', 'build', '--workspace'], capture_output=True)
    if result.returncode != 0:
        print("ERROR: Build failed after cleanup!")
        print(result.stderr.decode())
        return 1

    print("✓ Build successful")
    return 0

if __name__ == '__main__':
    sys.exit(main())
