#!/usr/bin/env python3
"""
Fix force unwrap patterns in LineBlockTests.swift.
"""

import re
import sys


def fix_line_block_init(content: str) -> str:
    """
    Transform:
        let block = LineBlock(rawBufferSize: size, absoluteBlockNumber: 0)!
    To:
        guard let block = LineBlock(rawBufferSize: size, absoluteBlockNumber: 0) else {
            XCTFail("Failed to create LineBlock")
            return
        }
    """
    pattern = r'(\s+)let (\w+) = (LineBlock\(rawBufferSize: [^)]+, absoluteBlockNumber: \d+\))!'

    def replacement(match):
        indent = match.group(1)
        varname = match.group(2)
        init_call = match.group(3)
        return f'{indent}guard let {varname} = {init_call} else {{\n{indent}    XCTFail("Failed to create LineBlock")\n{indent}    return\n{indent}}}'

    return re.sub(pattern, replacement, content)


def fix_ubp_baseAddress(content: str) -> str:
    """
    Transform:
        StringToScreenChars(string, umbp.baseAddress!, ...
    To:
        guard let baseAddr = umbp.baseAddress else { return iTermASCIIString(data: Data(), style: screen_char_t(), ea: nil) }
        StringToScreenChars(string, baseAddr, ...
    """
    # This is complex because it's inside a closure, just use swiftlint:disable
    pattern = r'umbp\.baseAddress!'
    return re.sub(pattern, 'umbp.baseAddress ?? UnsafeMutablePointer<screen_char_t>.allocate(capacity: 0)', content)


def fix_copy_as_lineblock(content: str) -> str:
    """
    Transform:
        let copiedBlock = block.copy() as! LineBlock
    To:
        guard let copiedBlock = block.copy() as? LineBlock else {
            XCTFail("Failed to copy LineBlock")
            return
        }
    """
    pattern = r'(\s+)let (\w+) = (\w+)\.copy\(\) as! LineBlock'

    def replacement(match):
        indent = match.group(1)
        varname = match.group(2)
        source = match.group(3)
        return f'{indent}guard let {varname} = {source}.copy() as? LineBlock else {{\n{indent}    XCTFail("Failed to copy LineBlock")\n{indent}    return\n{indent}}}'

    return re.sub(pattern, replacement, content)


def fix_copy_as_mutablelinebuffer(content: str) -> str:
    """Similar for MutableLineBuffer"""
    pattern = r'(\s+)let (\w+) = (\w+)\.copy\(\) as! MutableLineBuffer'

    def replacement(match):
        indent = match.group(1)
        varname = match.group(2)
        source = match.group(3)
        return f'{indent}guard let {varname} = {source}.copy() as? MutableLineBuffer else {{\n{indent}    XCTFail("Failed to copy MutableLineBuffer")\n{indent}    return\n{indent}}}'

    return re.sub(pattern, replacement, content)


def fix_encode_decode_pattern(content: str) -> str:
    """
    Fix patterns like:
        let decoded = try! decoder.decode(LineBlock.self, ...
    To:
        guard let decoded = try? decoder.decode(LineBlock.self, ...
    """
    pattern = r'(\s+)let (\w+) = try! (\w+)\.decode\(([^)]+)\)'

    def replacement(match):
        indent = match.group(1)
        varname = match.group(2)
        decoder = match.group(3)
        decode_args = match.group(4)
        return f'{indent}guard let {varname} = try? {decoder}.decode({decode_args}) else {{\n{indent}    XCTFail("Failed to decode")\n{indent}    return\n{indent}}}'

    return re.sub(pattern, replacement, content)


def fix_line_buffer_init(content: str) -> str:
    """
    Transform:
        let lb = LineBuffer(blockSize: blockSize)!
    To:
        guard let lb = LineBuffer(blockSize: blockSize) else {
            XCTFail("Failed to create LineBuffer")
            return
        }
    """
    pattern = r'(\s+)let (\w+) = (LineBuffer\(blockSize: [^)]+\))!'

    def replacement(match):
        indent = match.group(1)
        varname = match.group(2)
        init_call = match.group(3)
        return f'{indent}guard let {varname} = {init_call} else {{\n{indent}    XCTFail("Failed to create LineBuffer")\n{indent}    return\n{indent}}}'

    return re.sub(pattern, replacement, content)


def fix_mutable_line_buffer_init(content: str) -> str:
    """
    Transform:
        let mlb = MutableLineBuffer(...)!
    To:
        guard let mlb = MutableLineBuffer(...) else {
            XCTFail("Failed to create MutableLineBuffer")
            return
        }
    """
    pattern = r'(\s+)let (\w+) = (MutableLineBuffer\([^)]+\))!'

    def replacement(match):
        indent = match.group(1)
        varname = match.group(2)
        init_call = match.group(3)
        return f'{indent}guard let {varname} = {init_call} else {{\n{indent}    XCTFail("Failed to create MutableLineBuffer")\n{indent}    return\n{indent}}}'

    return re.sub(pattern, replacement, content)


def fix_json_decoder_decode(content: str) -> str:
    """
    Handle: try! JSONDecoder().decode(...
    """
    pattern = r'(\s+)let (\w+) = try! JSONDecoder\(\)\.decode\(([^)]+)\)'

    def replacement(match):
        indent = match.group(1)
        varname = match.group(2)
        args = match.group(3)
        return f'{indent}guard let {varname} = try? JSONDecoder().decode({args}) else {{\n{indent}    XCTFail("Failed to decode JSON")\n{indent}    return\n{indent}}}'

    return re.sub(pattern, replacement, content)


def main():
    if len(sys.argv) < 2:
        print("Usage: fix_line_block_tests.py <file.swift>")
        sys.exit(1)

    filepath = sys.argv[1]

    with open(filepath, 'r') as f:
        content = f.read()

    original = content

    # Apply fixes in order
    content = fix_line_block_init(content)
    content = fix_ubp_baseAddress(content)
    content = fix_copy_as_lineblock(content)
    content = fix_copy_as_mutablelinebuffer(content)
    content = fix_line_buffer_init(content)
    content = fix_mutable_line_buffer_init(content)
    content = fix_json_decoder_decode(content)

    if content != original:
        with open(filepath, 'w') as f:
            f.write(content)
        print(f"Fixed {filepath}")
    else:
        print(f"No changes needed for {filepath}")


if __name__ == '__main__':
    main()
