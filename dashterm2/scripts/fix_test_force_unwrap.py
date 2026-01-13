#!/usr/bin/env python3
"""
Fix force unwrap patterns in test files.
Transforms common patterns to use guard-let with XCTFail.
"""

import re
import sys


def fix_vt100grid_init(content: str) -> str:
    """
    Transform:
        let grid = VT100Grid(size: VT100GridSize(width: width, height: 4), delegate: nil)!
    To:
        guard let grid = VT100Grid(size: VT100GridSize(width: width, height: 4), delegate: nil) else {
            XCTFail("Failed to create VT100Grid")
            return
        }
    """
    # Pattern for VT100Grid initialization with force unwrap
    pattern = r'(\s+)let (\w+) = (VT100Grid\(size: [^)]+\), delegate: nil\))!'

    def replacement(match):
        indent = match.group(1)
        varname = match.group(2)
        init_call = match.group(3)
        return f'{indent}guard let {varname} = {init_call} else {{\n{indent}    XCTFail("Failed to create VT100Grid")\n{indent}    return\n{indent}}}'

    return re.sub(pattern, replacement, content)


def fix_screen_chars_at_line(content: str) -> str:
    """
    Transform:
        let line = grid.screenChars(atLineNumber: Int32(y))!
    To:
        guard let line = grid.screenChars(atLineNumber: Int32(y)) else {
            XCTFail("Failed to get screen chars")
            continue  // or return depending on context
        }
    """
    # Pattern for screenChars with force unwrap in loops (use continue)
    pattern = r'(\s+)let (\w+) = (\w+\.screenChars\(atLineNumber: [^)]+\))!'

    def replacement(match):
        indent = match.group(1)
        varname = match.group(2)
        call = match.group(3)
        return f'{indent}guard let {varname} = {call} else {{\n{indent}    XCTFail("Failed to get screen chars")\n{indent}    return\n{indent}}}'

    return re.sub(pattern, replacement, content)


def fix_nsmutabledata_init(content: str) -> str:
    """
    Transform:
        let data = NSMutableData(length: ...)!
    To:
        guard let data = NSMutableData(length: ...) else {
            XCTFail("Failed to create NSMutableData")
            return UnsafeMutablePointer<screen_char_t>.allocate(capacity: 0)
        }
    """
    pattern = r'(\s+)let (\w+) = (NSMutableData\(length: [^)]+\))!'

    def replacement(match):
        indent = match.group(1)
        varname = match.group(2)
        init_call = match.group(3)
        return f'{indent}guard let {varname} = {init_call} else {{\n{indent}    XCTFail("Failed to create NSMutableData")\n{indent}    return UnsafeMutablePointer<screen_char_t>.allocate(capacity: 0)\n{indent}}}'

    return re.sub(pattern, replacement, content)


def fix_character_utf16_first(content: str) -> str:
    """
    Transform:
        Character("x").utf16.first!
    To:
        (Character("x").utf16.first ?? 0)

    This is safe because single ASCII characters always have a utf16 value.
    """
    # Match Character("x").utf16.first! pattern
    pattern = r'Character\("([^"]+)"\)\.utf16\.first!'

    def replacement(match):
        char = match.group(1)
        return f'(Character("{char}").utf16.first ?? 0)'

    return re.sub(pattern, replacement, content)


def fix_c_utf16_first(content: str) -> str:
    """
    Transform:
        c.utf16.first!
    To:
        (c.utf16.first ?? 0)
    """
    pattern = r'\bc\.utf16\.first!'
    return re.sub(pattern, '(c.utf16.first ?? 0)', content)


def fix_as_force_cast(content: str) -> str:
    """
    Transform:
        let value = v as! NSValue
    To:
        guard let value = v as? NSValue else { continue }
    """
    # Simple in-loop force cast pattern
    pattern = r'(\s+)let (\w+) = (\w+) as! (\w+)'

    def replacement(match):
        indent = match.group(1)
        varname = match.group(2)
        source = match.group(3)
        target_type = match.group(4)
        return f'{indent}guard let {varname} = {source} as? {target_type} else {{ continue }}'

    return re.sub(pattern, replacement, content)


def fix_array_force_cast(content: str) -> str:
    """
    Transform:
        let rects = grid.rects(for: oneLineRun) as! [NSValue]
    To:
        guard let rects = grid.rects(for: oneLineRun) as? [NSValue] else {
            XCTFail("Failed to cast to [NSValue]")
            return
        }
    """
    pattern = r'(\s+)let (\w+) = ([^=]+) as! \[(\w+)\]'

    def replacement(match):
        indent = match.group(1)
        varname = match.group(2)
        expr = match.group(3).strip()
        element_type = match.group(4)
        return f'{indent}guard let {varname} = {expr} as? [{element_type}] else {{\n{indent}    XCTFail("Failed to cast to [{element_type}]")\n{indent}    return\n{indent}}}'

    return re.sub(pattern, replacement, content)


def fix_mutable_copy_cast(content: str) -> str:
    """
    Transform:
        let msca = sca.mutableCopy() as! MutableScreenCharArray
    To:
        guard let msca = sca.mutableCopy() as? MutableScreenCharArray else {
            XCTFail("Failed to create MutableScreenCharArray")
            return ""
        }
    """
    pattern = r'(\s+)let (\w+) = (\w+)\.mutableCopy\(\) as! (\w+)'

    def replacement(match):
        indent = match.group(1)
        varname = match.group(2)
        source = match.group(3)
        target_type = match.group(4)
        return f'{indent}guard let {varname} = {source}.mutableCopy() as? {target_type} else {{\n{indent}    XCTFail("Failed to create {target_type}")\n{indent}    return ""\n{indent}}}'

    return re.sub(pattern, replacement, content)


def fix_sca_force_unwrap(content: str) -> str:
    """
    Transform:
        let sca = screenCharArray(atLine: i)!
    To:
        guard let sca = screenCharArray(atLine: i) else {
            return ""
        }
    """
    pattern = r'(\s+)let (\w+) = (screenCharArray\(atLine: [^)]+\))!'

    def replacement(match):
        indent = match.group(1)
        varname = match.group(2)
        call = match.group(3)
        return f'{indent}guard let {varname} = {call} else {{\n{indent}    return ""\n{indent}}}'

    return re.sub(pattern, replacement, content)


def main():
    if len(sys.argv) < 2:
        print("Usage: fix_test_force_unwrap.py <file.swift>")
        sys.exit(1)

    filepath = sys.argv[1]

    with open(filepath, 'r') as f:
        content = f.read()

    original = content

    # Apply fixes in order
    content = fix_vt100grid_init(content)
    content = fix_screen_chars_at_line(content)
    content = fix_nsmutabledata_init(content)
    content = fix_character_utf16_first(content)
    content = fix_c_utf16_first(content)
    content = fix_array_force_cast(content)
    content = fix_mutable_copy_cast(content)
    content = fix_sca_force_unwrap(content)
    content = fix_as_force_cast(content)

    if content != original:
        with open(filepath, 'w') as f:
            f.write(content)
        print(f"Fixed {filepath}")
    else:
        print(f"No changes needed for {filepath}")


if __name__ == '__main__':
    main()
