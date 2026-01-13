#!/usr/bin/env python3
"""
Fix force unwrap patterns in VT100ScreenTests.swift.
"""

import re
import sys


def fix_mutableState_unwrap(content: str) -> str:
    """
    Transform:
        mutableState!.something
    To:
        guard let mutableState = mutableState else { return }
        mutableState.something

    This is tricky because mutableState! appears inside closures.
    We need to add swiftlint:disable for these since they're in performBlock closures.
    """
    # Replace mutableState! with mutableState only where it's safe
    # First, add guard at the top of closures where needed

    # Actually, since the closure type signature has `mutableState: VT100MutableState?`,
    # we can use optional chaining or guard
    # For now, just add swiftlint:disable:next for each line

    # This is complex - let's just do inline optional chaining
    return content


def fix_terminal_unwrap(content: str) -> str:
    """Fix mutableState!.terminal! patterns"""
    pattern = r'mutableState!\.terminal!'
    return re.sub(pattern, '(mutableState?.terminal ?? VT100Terminal())', content)


def fix_compactLineDump_unwrap(content: str) -> str:
    """Fix screen.compactLineDumpWithHistory()! patterns"""
    pattern = r'\.compactLineDumpWithHistory\(\)!'
    return re.sub(pattern, '.compactLineDumpWithHistory() ?? ""', content)


def fix_annotations_unwrap(content: str) -> str:
    """Fix screen.annotations(...)! patterns"""
    pattern = r'\.annotations\(in: ([^)]+)\)!'
    return re.sub(pattern, r'.annotations(in: \1) ?? []', content)


def fix_state_config_unwrap(content: str) -> str:
    """Fix state.config! patterns"""
    pattern = r'state\.config!'
    return re.sub(pattern, 'state.config ?? iTermMainThreadStateConfig()', content)


def main():
    if len(sys.argv) < 2:
        print("Usage: fix_vt100screen_tests.py <file.swift>")
        sys.exit(1)

    filepath = sys.argv[1]

    with open(filepath, 'r') as f:
        content = f.read()

    original = content

    content = fix_compactLineDump_unwrap(content)
    content = fix_annotations_unwrap(content)

    if content != original:
        with open(filepath, 'w') as f:
            f.write(content)
        print(f"Fixed {filepath}")
    else:
        print(f"No changes needed for {filepath}")


if __name__ == '__main__':
    main()
