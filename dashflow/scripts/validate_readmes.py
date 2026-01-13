#!/usr/bin/env python3
"""
README Validation Script - Part 9 Phase 201

Validates READMEs across the codebase for:
1. Missing READMEs in crate directories
2. Wrong content (title doesn't match directory)
3. Duplicate content across READMEs
4. Version consistency (should reference 1.11)
5. Placeholder content detection

Usage:
    python3 scripts/validate_readmes.py
    python3 scripts/validate_readmes.py --fix  # Future: auto-fix issues
"""

import os
import sys
import hashlib
from pathlib import Path
from collections import defaultdict

WORKSPACE_VERSION = "1.11"
REPO_ROOT = Path(__file__).parent.parent


def get_crate_directories():
    """Find all crate directories under crates/."""
    crates_dir = REPO_ROOT / "crates"
    return [d for d in crates_dir.iterdir() if d.is_dir() and (d / "Cargo.toml").exists()]


def get_readme_content(crate_dir):
    """Read README.md content if it exists."""
    readme_path = crate_dir / "README.md"
    if readme_path.exists():
        return readme_path.read_text()
    return None


def check_missing_readmes(crates):
    """Find crates without READMEs."""
    missing = []
    for crate in crates:
        if not (crate / "README.md").exists():
            missing.append(crate.name)
    return missing


def check_title_mismatch(crates):
    """Find READMEs where title doesn't match crate name."""
    mismatches = []
    for crate in crates:
        content = get_readme_content(crate)
        if content:
            lines = content.strip().split('\n')
            if lines:
                title = lines[0].lstrip('#').strip()
                # Allow "DashFlow X" style titles for detailed READMEs
                expected_names = [crate.name, crate.name.replace('-', ' ').title()]
                if title not in expected_names and not title.lower().startswith(crate.name.replace('-', ' ')):
                    mismatches.append((crate.name, title))
    return mismatches


def check_duplicate_content(crates):
    """Find READMEs with identical or very similar content."""
    content_hashes = defaultdict(list)
    for crate in crates:
        content = get_readme_content(crate)
        if content:
            # Hash the content after stripping the first line (title may differ)
            lines = content.strip().split('\n')
            if len(lines) > 1:
                body = '\n'.join(lines[1:]).strip()
                content_hash = hashlib.md5(body.encode()).hexdigest()
                content_hashes[content_hash].append(crate.name)

    # Find hashes with multiple crates (duplicates)
    duplicates = {h: crates for h, crates in content_hashes.items() if len(crates) > 1}
    return duplicates


def check_version_consistency(crates):
    """Find READMEs with outdated dashflow version references."""
    outdated = []
    for crate in crates:
        content = get_readme_content(crate)
        if content:
            # Only check dashflow crate versions, not dependencies like serde
            import re
            # Match patterns like: dashflow-xyz = "1.X" or dashflow = "1.X"
            dashflow_versions = re.findall(r'dashflow[a-z-]* = "(\d+\.\d+)"', content)
            for version in dashflow_versions:
                if version.startswith('1.') and version != WORKSPACE_VERSION:
                    outdated.append((crate.name, version))
                    break
    return outdated


def check_placeholder_content(crates):
    """Find READMEs that are just placeholder templates."""
    placeholders = []
    for crate in crates:
        content = get_readme_content(crate)
        if content:
            lines = content.strip().split('\n')
            # Minimal template has exactly ~16 lines with generic content
            if len(lines) <= 20:
                # Check for generic placeholder text
                if "integration for DashFlow" in content and "Will be available on docs.rs" in content:
                    placeholders.append(crate.name)
    return placeholders


def main():
    """Run all README validations."""
    print("=== DashFlow README Validation ===\n")

    crates = get_crate_directories()
    print(f"Found {len(crates)} crate directories\n")

    issues_found = False

    # 1. Missing READMEs
    missing = check_missing_readmes(crates)
    if missing:
        issues_found = True
        print(f"❌ Missing READMEs ({len(missing)}):")
        for name in missing:
            print(f"   - {name}")
        print()
    else:
        print("✓ All crates have READMEs")

    # 2. Title mismatches
    mismatches = check_title_mismatch(crates)
    if mismatches:
        issues_found = True
        print(f"\n❌ Title Mismatches ({len(mismatches)}):")
        for name, title in mismatches:
            print(f"   - {name}: found '{title}'")
        print()
    else:
        print("✓ All README titles match crate names")

    # 3. Version consistency
    outdated = check_version_consistency(crates)
    if outdated:
        issues_found = True
        print(f"\n❌ Outdated Versions ({len(outdated)}):")
        for name, version in outdated:
            print(f"   - {name}: has {version}, expected {WORKSPACE_VERSION}")
        print()
    else:
        print("✓ All versions are consistent")

    # 4. Duplicate content (informational only)
    duplicates = check_duplicate_content(crates)
    if duplicates:
        dup_count = sum(len(crates) for crates in duplicates.values())
        print(f"\nℹ️  Similar Content ({dup_count} READMEs in {len(duplicates)} groups):")
        for crates in duplicates.values():
            if len(crates) > 5:  # Only show if many duplicates
                print(f"   - {len(crates)} crates share similar content")

    # 5. Placeholder content (informational only)
    placeholders = check_placeholder_content(crates)
    if placeholders:
        print(f"\nℹ️  Minimal/Placeholder READMEs ({len(placeholders)}):")
        print(f"   (These are valid but could be enhanced)")

    print("\n=== Validation Complete ===")

    if issues_found:
        print("\nFix required issues before committing.")
        return 1
    else:
        print("\nAll checks passed!")
        return 0


if __name__ == "__main__":
    sys.exit(main())
