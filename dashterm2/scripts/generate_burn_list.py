#!/usr/bin/env python3
"""
Generate burn list from upstream issues.
Creates a file tree with master index and category-specific files.
"""

import re
import os
from datetime import datetime
from pathlib import Path

# Configuration
UPSTREAM_FILE = "docs/UPSTREAM-ISSUES.md"
BURN_LIST_DIR = "docs/burn-list"
MASTER_INDEX = f"{BURN_LIST_DIR}/README.md"

# Category mappings for filenames (handle variations in source file)
CATEGORY_SLUGS = {
    "Crashes and Hangs": "P0-crashes-hangs",
    "AI Integration": "P1-ai-integration",
    "tmux Integration": "P1-tmux",
    "SSH/SFTP/SCP": "P1-ssh-sftp-scp",
    "Shell Integration": "P1-shell-integration",
    "Performance": "P2-performance",
    "Scrollback": "P2-scrollback",
    "Font and Rendering": "P2-font-rendering",
    "Window/Tab/Pane": "P2-window-tab-pane",
    "Color and Theme": "P3-color-theme",
    "Browser Integration": "P3-browser",
    "Keyboard and Input": "P2-keyboard-input",
    "Profile and Settings": "P3-profile-settings",
    "AppleScript and API": "P3-applescript-api",
    "macOS Version Specific": "P2-macos-version",
    "Copy/Paste/Selection": "P2-copy-paste-select",
    "Other Issues": "P3-other",
}

PRIORITY_MAP = {
    "Crashes and Hangs": "P0",
    "AI Integration": "P1",
    "tmux Integration": "P1",
    "SSH/SFTP/SCP": "P1",
    "Shell Integration": "P1",
    "Performance": "P2",
    "Scrollback": "P2",
    "Font and Rendering": "P2",
    "Window/Tab/Pane": "P2",
    "Color and Theme": "P3",
    "Browser Integration": "P3",
    "Keyboard and Input": "P2",
    "Profile and Settings": "P3",
    "AppleScript and API": "P3",
    "macOS Version Specific": "P2",
    "Copy/Paste/Selection": "P2",
    "Other Issues": "P3",
}


def parse_upstream_issues(filepath):
    """Parse the upstream issues markdown file."""
    with open(filepath, 'r') as f:
        content = f.read()

    categories = {}
    current_category = None
    current_issues = []

    # Pattern to match category headers - more flexible
    # Matches: ## Category Name (P0 - CRITICAL) or ## Category Name (P1) etc
    category_pattern = re.compile(r'^## ([^(]+?) \(P\d+')
    # Pattern to match issue lines
    issue_pattern = re.compile(r'\| \[#(\d+)\]\(([^)]+)\) \| (.+?) \|')

    lines = content.split('\n')

    for line in lines:
        # Check for category header
        cat_match = category_pattern.match(line)
        if cat_match:
            # Save previous category
            if current_category and current_issues:
                categories[current_category] = current_issues
            current_category = cat_match.group(1).strip()
            current_issues = []
            continue

        # Check for issue line
        issue_match = issue_pattern.search(line)
        if issue_match and current_category:
            issue_num = issue_match.group(1)
            issue_url = issue_match.group(2)
            issue_title = issue_match.group(3).strip()
            current_issues.append({
                'number': issue_num,
                'url': issue_url,
                'title': issue_title,
            })

    # Save last category
    if current_category and current_issues:
        categories[current_category] = current_issues

    return categories


def generate_master_index(categories, output_dir):
    """Generate the master index README.md."""
    now = datetime.now().strftime("%Y-%m-%d %H:%M")

    total_issues = sum(len(issues) for issues in categories.values())

    content = f"""# DashTerm2 Bug Burn List

**Generated:** {now}
**Total Issues:** {total_issues}
**Source:** [Upstream iTerm2 GitLab](https://gitlab.com/gnachman/iterm2/-/issues)

---

## Progress Summary

| Priority | Category | Total | Inspected | Fixed | In Progress | Remaining | Progress |
|----------|----------|-------|-----------|-------|-------------|-----------|----------|
"""

    # Sort categories by priority
    priority_order = ['P0', 'P1', 'P2', 'P3']
    sorted_cats = sorted(categories.keys(),
                         key=lambda c: (priority_order.index(PRIORITY_MAP.get(c, 'P3')), c))

    for cat in sorted_cats:
        issues = categories[cat]
        count = len(issues)
        slug = CATEGORY_SLUGS.get(cat, 'P3-' + cat.lower().replace(' ', '-').replace('/', '-'))
        priority = PRIORITY_MAP.get(cat, 'P3')
        content += f"| {priority} | [{cat}](./{slug}.md) | {count} | 0 | 0 | 0 | {count} | 0% |\n"

    content += f"| | **TOTAL** | **{total_issues}** | **0** | **0** | **0** | **{total_issues}** | **0%** |\n"

    content += """
---

## Priority Legend

| Priority | Description | SLA | Action |
|----------|-------------|-----|--------|
| **P0** | Crashes, hangs, data loss | Fix immediately | Drop everything |
| **P1** | Core functionality broken (tmux, SSH, AI) | Fix this sprint | High priority |
| **P2** | Important but workarounds exist | Fix this month | Normal priority |
| **P3** | Nice to have, minor issues | Fix when possible | Low priority |

---

## Category Files

### P0 - Critical (Fix Immediately)
"""

    for cat in sorted_cats:
        priority = PRIORITY_MAP.get(cat, 'P3')
        if priority != 'P0':
            continue
        slug = CATEGORY_SLUGS.get(cat, 'P3-' + cat.lower().replace(' ', '-').replace('/', '-'))
        count = len(categories[cat])
        content += f"- [{cat}](./{slug}.md) - **{count} issues**\n"

    content += "\n### P1 - High Priority (This Sprint)\n"
    for cat in sorted_cats:
        priority = PRIORITY_MAP.get(cat, 'P3')
        if priority != 'P1':
            continue
        slug = CATEGORY_SLUGS.get(cat, 'P3-' + cat.lower().replace(' ', '-').replace('/', '-'))
        count = len(categories[cat])
        content += f"- [{cat}](./{slug}.md) - {count} issues\n"

    content += "\n### P2 - Medium Priority (This Month)\n"
    for cat in sorted_cats:
        priority = PRIORITY_MAP.get(cat, 'P3')
        if priority != 'P2':
            continue
        slug = CATEGORY_SLUGS.get(cat, 'P3-' + cat.lower().replace(' ', '-').replace('/', '-'))
        count = len(categories[cat])
        content += f"- [{cat}](./{slug}.md) - {count} issues\n"

    content += "\n### P3 - Low Priority (When Possible)\n"
    for cat in sorted_cats:
        priority = PRIORITY_MAP.get(cat, 'P3')
        if priority != 'P3':
            continue
        slug = CATEGORY_SLUGS.get(cat, 'P3-' + cat.lower().replace(' ', '-').replace('/', '-'))
        count = len(categories[cat])
        content += f"- [{cat}](./{slug}.md) - {count} issues\n"

    content += """
---

## Workflow

### For Workers

1. **Pick issues from P0 first**, then P1, P2, P3
2. **Inspect the issue** - read GitLab, understand root cause
3. **Update `Date Inspected`** when you start investigating
4. **Fix the bug** with proper root cause analysis
5. **Write a test** that proves the fix works
6. **Update the table** with all fields filled in
7. **Commit** with message referencing the issue

### Column Definitions

| Column | Description |
|--------|-------------|
| ID | GitLab issue number (link to upstream) |
| Title | Brief title from GitLab |
| Description | One-line summary of the bug |
| Date Inspected | When a worker first looked at this |
| Date Fixed | When the fix was merged |
| Commits | Commit hashes that fixed this |
| Tests | Test names that verify the fix |
| Status | Open, Inspected, In Progress, Fixed, Wontfix |
| Notes | Blockers, related issues, etc. |

### Status Values

| Status | Meaning |
|--------|---------|
| `Open` | Not started |
| `Inspected` | Investigated but not yet fixing |
| `In Progress` | Actively being fixed |
| `Fixed` | Merged to main with test |
| `Wontfix` | Decided not to fix (document why) |
| `Duplicate` | Duplicate of another issue |
| `Upstream` | Waiting on upstream iTerm2 fix |
| `Cannot Reproduce` | Unable to reproduce the issue |

"""

    return content


def generate_category_file(category, issues, output_dir):
    """Generate a category-specific markdown file."""
    now = datetime.now().strftime("%Y-%m-%d %H:%M")
    priority = PRIORITY_MAP.get(category, 'P3')

    content = f"""# {category}

**Priority:** {priority}
**Total Issues:** {len(issues)}
**Fixed:** 0
**In Progress:** 0
**Remaining:** {len(issues)}
**Last Updated:** {now}

[< Back to Master Index](./README.md)

---

## Issues

| ID | Title | Description | Date Inspected | Date Fixed | Commits | Tests | Status | Notes |
|----|-------|-------------|----------------|------------|---------|-------|--------|-------|
"""

    for issue in issues:
        # Escape pipe characters in title
        title = issue['title'].replace('|', '\\|')
        # Truncate long titles for the table
        if len(title) > 60:
            short_title = title[:57] + "..."
        else:
            short_title = title

        content += f"| [#{issue['number']}]({issue['url']}) | {short_title} | - | - | - | - | - | Open | - |\n"

    content += f"""
---

## Statistics

| Metric | Count |
|--------|-------|
| Total | {len(issues)} |
| Fixed | 0 |
| In Progress | 0 |
| Inspected | 0 |
| Open | {len(issues)} |
| Wontfix | 0 |

---

## Category Notes

_Add notes specific to {category} bugs here._

### Common Patterns

_Document common root causes or fix patterns for this category._

### Related Files

_List source files commonly involved in these bugs._

"""

    return content


def main():
    # Get repo root
    script_dir = Path(__file__).parent
    repo_root = script_dir.parent

    upstream_path = repo_root / UPSTREAM_FILE
    burn_list_dir = repo_root / BURN_LIST_DIR

    print(f"Parsing upstream issues from {upstream_path}...")
    categories = parse_upstream_issues(upstream_path)

    total = sum(len(issues) for issues in categories.values())
    print(f"Found {len(categories)} categories with {total} total issues:")
    for cat, issues in sorted(categories.items(), key=lambda x: -len(x[1])):
        priority = PRIORITY_MAP.get(cat, 'P3')
        print(f"  [{priority}] {cat}: {len(issues)} issues")

    # Create output directory
    burn_list_dir.mkdir(parents=True, exist_ok=True)
    print(f"\nCreating burn list in {burn_list_dir}/")

    # Generate master index
    master_content = generate_master_index(categories, burn_list_dir)
    master_path = burn_list_dir / "README.md"
    with open(master_path, 'w') as f:
        f.write(master_content)
    print(f"  Created {master_path}")

    # Generate category files
    for category, issues in categories.items():
        slug = CATEGORY_SLUGS.get(category, 'P3-' + category.lower().replace(' ', '-').replace('/', '-'))
        cat_content = generate_category_file(category, issues, burn_list_dir)
        cat_path = burn_list_dir / f"{slug}.md"
        with open(cat_path, 'w') as f:
            f.write(cat_content)
        print(f"  Created {cat_path} ({len(issues)} issues)")

    print(f"\nDone! Created {len(categories) + 1} files in {burn_list_dir}/")
    print(f"Master index: {master_path}")


if __name__ == "__main__":
    main()
