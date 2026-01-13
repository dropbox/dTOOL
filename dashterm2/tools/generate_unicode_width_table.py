#!/usr/bin/env python3
"""
Generate static Unicode width lookup tables for DashTerm2.

This script creates a two-level lookup table for fast O(1) character width
classification. The output is a C source file with static arrays.

Table structure:
- Stage 1: Maps codepoint blocks (codepoint >> 8) to Stage 2 offsets
- Stage 2: Per-block lookup (256 entries) with width flags

Memory usage: ~70KB total (vs ~500KB+ for NSCharacterSet instances)
Lookup time: ~2-5ns (vs ~50-100ns for NSCharacterSet)
"""

import itertools
import sys

# Width flag bits (must match iTermUnicodeWidthTable.h)
WIDTH_NONE = 0
WIDTH_FULL_V8 = 1 << 0
WIDTH_FULL_V9 = 1 << 1
WIDTH_AMBIGUOUS_V8 = 1 << 2
WIDTH_AMBIGUOUS_V9 = 1 << 3

# Maximum Unicode codepoint we care about
MAX_CODEPOINT = 0x10FFFF

# Full-width ranges for Unicode 8
# From NSCharacterSet+iTerm.m sFullWidth8
FULL_WIDTH_V8_RANGES = [
    (0x1100, 0x115f),
    (0x11a3, 0x11a7),
    (0x11fa, 0x11ff),
    (0x2329, 0x232a),
    (0x2e80, 0x2e99),
    (0x2e9b, 0x2ef3),
    (0x2f00, 0x2fd5),
    (0x2ff0, 0x2ffb),
    (0x3000, 0x303e),
    (0x3041, 0x3096),
    (0x3099, 0x30ff),
    (0x3105, 0x312d),
    (0x3131, 0x318e),
    (0x3190, 0x31ba),
    (0x31c0, 0x31e3),
    (0x31f0, 0x321e),
    (0x3220, 0x3247),
    (0x3250, 0x32fe),
    (0x3300, 0x4dbf),
    (0x4e00, 0xa48c),
    (0xa490, 0xa4c6),
    (0xa960, 0xa97c),
    (0xac00, 0xd7a3),
    (0xd7b0, 0xd7c6),
    (0xd7cb, 0xd7fb),
    (0xf900, 0xfaff),
    (0xfe10, 0xfe19),
    (0xfe30, 0xfe52),
    (0xfe54, 0xfe66),
    (0xfe68, 0xfe6b),
    (0xff01, 0xff60),
    (0xffe0, 0xffe6),
    (0x1b000, 0x1b001),
    (0x1f200, 0x1f202),
    (0x1f210, 0x1f23a),
    (0x1f240, 0x1f248),
    (0x1f250, 0x1f251),
    (0x20000, 0x2fffd),
    (0x30000, 0x3fffd),
]

# Full-width ranges for Unicode 9+ (from EastAsianWidth.txt)
# This is a simplified version - the actual data should be generated from
# the Unicode consortium's data files
FULL_WIDTH_V9_RANGES = [
    (0x1100, 0x115f),
    (0x231a, 0x231b),
    (0x2329, 0x232a),
    (0x23e9, 0x23ec),
    (0x23f0, 0x23f0),
    (0x23f3, 0x23f3),
    (0x25fd, 0x25fe),
    (0x2614, 0x2615),
    (0x2630, 0x2637),
    (0x2648, 0x2653),
    (0x267f, 0x267f),
    (0x268a, 0x268f),
    (0x2693, 0x2693),
    (0x26a1, 0x26a1),
    (0x26aa, 0x26ab),
    (0x26bd, 0x26be),
    (0x26c4, 0x26c5),
    (0x26ce, 0x26ce),
    (0x26d4, 0x26d4),
    (0x26ea, 0x26ea),
    (0x26f2, 0x26f3),
    (0x26f5, 0x26f5),
    (0x26fa, 0x26fa),
    (0x26fd, 0x26fd),
    (0x2705, 0x2705),
    (0x270a, 0x270b),
    (0x2728, 0x2728),
    (0x274c, 0x274c),
    (0x274e, 0x274e),
    (0x2753, 0x2755),
    (0x2757, 0x2757),
    (0x2795, 0x2797),
    (0x27b0, 0x27b0),
    (0x27bf, 0x27bf),
    (0x2b1b, 0x2b1c),
    (0x2b50, 0x2b50),
    (0x2b55, 0x2b55),
    (0x2e80, 0x2e99),
    (0x2e9b, 0x2ef3),
    (0x2f00, 0x2fd5),
    (0x2ff0, 0x303e),
    (0x3041, 0x3096),
    (0x3099, 0x30ff),
    (0x3105, 0x312f),
    (0x3131, 0x318e),
    (0x3190, 0x31e3),
    (0x31f0, 0x321e),
    (0x3220, 0x3247),
    (0x3250, 0x4dbf),
    (0x4e00, 0x9fff),
    (0xa000, 0xa48c),
    (0xa490, 0xa4c6),
    (0xa960, 0xa97c),
    (0xac00, 0xd7a3),
    (0xd7b0, 0xd7c6),
    (0xd7cb, 0xd7fb),
    (0xf900, 0xfaff),
    (0xfe10, 0xfe19),
    (0xfe30, 0xfe52),
    (0xfe54, 0xfe66),
    (0xfe68, 0xfe6b),
    (0xff01, 0xff60),
    (0xffe0, 0xffe6),
    (0x1b000, 0x1b0ff),
    (0x1b100, 0x1b12f),
    (0x1f004, 0x1f004),
    (0x1f0cf, 0x1f0cf),
    (0x1f18e, 0x1f18e),
    (0x1f191, 0x1f19a),
    (0x1f200, 0x1f202),
    (0x1f210, 0x1f23b),
    (0x1f240, 0x1f248),
    (0x1f250, 0x1f251),
    (0x1f260, 0x1f265),
    (0x1f300, 0x1f320),
    (0x1f32d, 0x1f335),
    (0x1f337, 0x1f37c),
    (0x1f37e, 0x1f393),
    (0x1f3a0, 0x1f3ca),
    (0x1f3cf, 0x1f3d3),
    (0x1f3e0, 0x1f3f0),
    (0x1f3f4, 0x1f3f4),
    (0x1f3f8, 0x1f43e),
    (0x1f440, 0x1f440),
    (0x1f442, 0x1f4fc),
    (0x1f4ff, 0x1f53d),
    (0x1f54b, 0x1f54e),
    (0x1f550, 0x1f567),
    (0x1f57a, 0x1f57a),
    (0x1f595, 0x1f596),
    (0x1f5a4, 0x1f5a4),
    (0x1f5fb, 0x1f64f),
    (0x1f680, 0x1f6c5),
    (0x1f6cc, 0x1f6cc),
    (0x1f6d0, 0x1f6d2),
    (0x1f6d5, 0x1f6d7),
    (0x1f6eb, 0x1f6ec),
    (0x1f6f4, 0x1f6fc),
    (0x1f7e0, 0x1f7eb),
    (0x1f90c, 0x1f93a),
    (0x1f93c, 0x1f945),
    (0x1f947, 0x1f978),
    (0x1f97a, 0x1f9cb),
    (0x1f9cd, 0x1f9ff),
    (0x1fa00, 0x1fa53),
    (0x1fa60, 0x1fa6d),
    (0x1fa70, 0x1fa74),
    (0x1fa78, 0x1fa7a),
    (0x1fa80, 0x1fa86),
    (0x1fa90, 0x1faa8),
    (0x1fab0, 0x1fab6),
    (0x1fac0, 0x1fac2),
    (0x1fad0, 0x1fad6),
    (0x20000, 0x2fffd),
    (0x30000, 0x3fffd),
]

# Ambiguous-width ranges (simplified - see NSCharacterSet+iTerm.m for full list)
AMBIGUOUS_V8_RANGES = [
    (0xa1, 0xa1),
    (0xa4, 0xa4),
    (0xa7, 0xa8),
    (0xaa, 0xaa),
    (0xad, 0xae),
    (0xb0, 0xb4),
    (0xb6, 0xba),
    (0xbc, 0xbf),
    (0xc6, 0xc6),
    (0xd0, 0xd0),
    (0xd7, 0xd8),
    (0xde, 0xe1),
    (0xe6, 0xe6),
    (0xe8, 0xea),
    (0xec, 0xed),
    (0xf0, 0xf0),
    (0xf2, 0xf3),
    (0xf7, 0xfa),
    (0xfc, 0xfc),
    (0xfe, 0xfe),
    (0x101, 0x101),
    (0x111, 0x111),
    (0x113, 0x113),
    (0x11b, 0x11b),
    (0x126, 0x127),
    (0x12b, 0x12b),
    (0x131, 0x133),
    (0x138, 0x138),
    (0x13f, 0x142),
    (0x144, 0x144),
    (0x148, 0x14b),
    (0x14d, 0x14d),
    (0x152, 0x153),
    (0x166, 0x167),
    (0x16b, 0x16b),
    (0x1ce, 0x1ce),
    (0x1d0, 0x1d0),
    (0x1d2, 0x1d2),
    (0x1d4, 0x1d4),
    (0x1d6, 0x1d6),
    (0x1d8, 0x1d8),
    (0x1da, 0x1da),
    (0x1dc, 0x1dc),
    (0x251, 0x251),
    (0x261, 0x261),
    (0x2c4, 0x2c4),
    (0x2c7, 0x2c7),
    (0x2c9, 0x2cb),
    (0x2cd, 0x2cd),
    (0x2d0, 0x2d0),
    (0x2d8, 0x2db),
    (0x2dd, 0x2dd),
    (0x2df, 0x2df),
    (0x300, 0x36f),
    (0x391, 0x3a1),
    (0x3a3, 0x3a9),
    (0x3b1, 0x3c1),
    (0x3c3, 0x3c9),
    (0x401, 0x401),
    (0x410, 0x44f),
    (0x451, 0x451),
    (0x2010, 0x2010),
    (0x2013, 0x2016),
    (0x2018, 0x2019),
    (0x201c, 0x201d),
    (0x2020, 0x2022),
    (0x2024, 0x2027),
    (0x2030, 0x2030),
    (0x2032, 0x2033),
    (0x2035, 0x2035),
    (0x203b, 0x203b),
    (0x203e, 0x203e),
    (0x2074, 0x2074),
    (0x207f, 0x207f),
    (0x2081, 0x2084),
    (0x20ac, 0x20ac),
    (0x2103, 0x2103),
    (0x2105, 0x2105),
    (0x2109, 0x2109),
    (0x2113, 0x2113),
    (0x2116, 0x2116),
    (0x2121, 0x2122),
    (0x2126, 0x2126),
    (0x212b, 0x212b),
    (0x2153, 0x2154),
    (0x215b, 0x215e),
    (0x2160, 0x216b),
    (0x2170, 0x2179),
    (0x2189, 0x2189),
    (0x2190, 0x2199),
    (0x21b8, 0x21b9),
    (0x21d2, 0x21d2),
    (0x21d4, 0x21d4),
    (0x21e7, 0x21e7),
    (0x2200, 0x2200),
    (0x2202, 0x2203),
    (0x2207, 0x2208),
    (0x220b, 0x220b),
    (0x220f, 0x220f),
    (0x2211, 0x2211),
    (0x2215, 0x2215),
    (0x221a, 0x221a),
    (0x221d, 0x2220),
    (0x2223, 0x2223),
    (0x2225, 0x2225),
    (0x2227, 0x222c),
    (0x222e, 0x222e),
    (0x2234, 0x2237),
    (0x223c, 0x223d),
    (0x2248, 0x2248),
    (0x224c, 0x224c),
    (0x2252, 0x2252),
    (0x2260, 0x2261),
    (0x2264, 0x2267),
    (0x226a, 0x226b),
    (0x226e, 0x226f),
    (0x2282, 0x2283),
    (0x2286, 0x2287),
    (0x2295, 0x2295),
    (0x2299, 0x2299),
    (0x22a5, 0x22a5),
    (0x22bf, 0x22bf),
    (0x2312, 0x2312),
    (0x2460, 0x24e9),
    (0x24eb, 0x254b),
    (0x2550, 0x2573),
    (0x2580, 0x258f),
    (0x2592, 0x2595),
    (0x25a0, 0x25a1),
    (0x25a3, 0x25a9),
    (0x25b2, 0x25b3),
    (0x25b6, 0x25b7),
    (0x25bc, 0x25bd),
    (0x25c0, 0x25c1),
    (0x25c6, 0x25c8),
    (0x25cb, 0x25cb),
    (0x25ce, 0x25d1),
    (0x25e2, 0x25e5),
    (0x25ef, 0x25ef),
    (0x2605, 0x2606),
    (0x2609, 0x2609),
    (0x260e, 0x260f),
    (0x2614, 0x2615),
    (0x261c, 0x261c),
    (0x261e, 0x261e),
    (0x2640, 0x2640),
    (0x2642, 0x2642),
    (0x2660, 0x2661),
    (0x2663, 0x2665),
    (0x2667, 0x266a),
    (0x266c, 0x266d),
    (0x266f, 0x266f),
    (0x269e, 0x269f),
    (0x26be, 0x26bf),
    (0x26c4, 0x26cd),
    (0x26cf, 0x26e1),
    (0x26e3, 0x26e3),
    (0x26e8, 0x26ff),
    (0x273d, 0x273d),
    (0x2757, 0x2757),
    (0x2776, 0x277f),
    (0x2b55, 0x2b59),
    (0x3248, 0x324f),
    (0xe000, 0xf8ff),
    (0xfe00, 0xfe0f),
    (0xfffd, 0xfffd),
    (0x1f100, 0x1f10a),
    (0x1f110, 0x1f12d),
    (0x1f130, 0x1f169),
    (0x1f170, 0x1f19a),
    (0xe0100, 0xe01ef),
    (0xf0000, 0xffffd),
    (0x100000, 0x10fffd),
]

# Ambiguous V9 is similar to V8 with some updates
AMBIGUOUS_V9_RANGES = AMBIGUOUS_V8_RANGES  # Use same for now

def ranges_to_set(ranges):
    """Convert list of (start, end) tuples to a set of codepoints."""
    result = set()
    for start, end in ranges:
        for cp in range(start, end + 1):
            result.add(cp)
    return result

def generate_tables():
    """Generate the two-level lookup tables."""

    # Build the codepoint to flags mapping
    cp_flags = {}

    full_v8 = ranges_to_set(FULL_WIDTH_V8_RANGES)
    full_v9 = ranges_to_set(FULL_WIDTH_V9_RANGES)
    ambig_v8 = ranges_to_set(AMBIGUOUS_V8_RANGES)
    ambig_v9 = ranges_to_set(AMBIGUOUS_V9_RANGES)

    # Collect all relevant codepoints
    all_cps = full_v8 | full_v9 | ambig_v8 | ambig_v9

    for cp in all_cps:
        flags = 0
        if cp in full_v8:
            flags |= WIDTH_FULL_V8
        if cp in full_v9:
            flags |= WIDTH_FULL_V9
        if cp in ambig_v8:
            flags |= WIDTH_AMBIGUOUS_V8
        if cp in ambig_v9:
            flags |= WIDTH_AMBIGUOUS_V9
        cp_flags[cp] = flags

    # Determine which blocks have any flags
    blocks_with_data = set()
    for cp in cp_flags:
        blocks_with_data.add(cp >> 8)

    # Build Stage 2 data - each block is 256 bytes
    stage2_blocks = []  # List of 256-byte arrays
    block_to_offset = {}  # block index -> stage2 offset

    # Special "all zeros" block for blocks with no data
    # We'll use 0xFFFF in stage1 to indicate "no data"

    for block_idx in sorted(blocks_with_data):
        block_data = [0] * 256
        for offset in range(256):
            cp = (block_idx << 8) | offset
            if cp in cp_flags:
                block_data[offset] = cp_flags[cp]

        # Check if this block is all zeros (shouldn't happen if block_idx is in blocks_with_data)
        if any(block_data):
            block_to_offset[block_idx] = len(stage2_blocks) * 256
            stage2_blocks.append(block_data)

    # Build Stage 1 - map block index to stage2 offset (or 0xFFFF for no data)
    max_block = max(blocks_with_data) if blocks_with_data else 0
    stage1 = [0xFFFF] * (max_block + 1)
    for block_idx, offset in block_to_offset.items():
        stage1[block_idx] = offset

    # Flatten stage2
    stage2 = []
    for block_data in stage2_blocks:
        stage2.extend(block_data)

    return stage1, stage2

def format_c_array(name, data, items_per_line=16, dtype='uint8_t'):
    """Format data as a C array declaration."""
    lines = [f"const {dtype} {name}[] = {{"]

    for i in range(0, len(data), items_per_line):
        chunk = data[i:i + items_per_line]
        if dtype == 'uint16_t':
            formatted = ", ".join(f"0x{x:04x}" for x in chunk)
        else:
            formatted = ", ".join(f"0x{x:02x}" for x in chunk)
        lines.append(f"    {formatted},")

    lines.append("};")
    return "\n".join(lines)

def main():
    stage1, stage2 = generate_tables()

    print("""//
//  iTermUnicodeWidthTable.m
//  DashTerm2
//
//  Auto-generated Unicode width lookup tables.
//  DO NOT EDIT - regenerate with tools/generate_unicode_width_table.py
//
//  Generated: 2025-12-17
//

#import "iTermUnicodeWidthTable.h"

// Stage 1: Maps codepoint blocks (codepoint >> 8) to Stage 2 offsets.
// 0xFFFF indicates the block has no width-modified characters.""")
    print(format_c_array("iTermUnicodeWidthStage1", stage1, items_per_line=16, dtype='uint16_t'))
    print(f"\nconst uint32_t iTermUnicodeWidthStage1Size = {len(stage1)};")

    print("""
// Stage 2: Per-block width flags (256 bytes per block).
// Each byte contains iTermUnicodeWidthFlags for one codepoint.""")
    print(format_c_array("iTermUnicodeWidthStage2", stage2, items_per_line=16, dtype='uint8_t'))
    print(f"\nconst uint32_t iTermUnicodeWidthStage2Size = {len(stage2)};")

    print("""
#pragma mark - Objective-C Interface

@implementation iTermUnicodeWidthTable

+ (iTermUnicodeWidthTable *)sharedInstance {
    static iTermUnicodeWidthTable *instance;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        instance = [[iTermUnicodeWidthTable alloc] init];
    });
    return instance;
}

- (BOOL)isFullWidth:(UTF32Char)codepoint unicodeVersion:(NSInteger)version {
    return iTermIsFullWidthFast(codepoint, (int)version) != 0;
}

- (BOOL)isAmbiguousWidth:(UTF32Char)codepoint unicodeVersion:(NSInteger)version {
    return iTermIsAmbiguousWidthFast(codepoint, (int)version) != 0;
}

- (iTermUnicodeWidthFlags)widthFlagsForCodepoint:(UTF32Char)codepoint {
    return iTermGetWidthFlagsFast(codepoint);
}

+ (BOOL)isDoubleWidthCharacter:(int)unicode
        ambiguousIsDoubleWidth:(BOOL)ambiguousIsDoubleWidth
                unicodeVersion:(NSInteger)version
                fullWidthFlags:(BOOL)fullWidthFlags {
    // TODO: Handle fullWidthFlags for flag characters
    return iTermIsDoubleWidthFast((UTF32Char)unicode,
                                   ambiguousIsDoubleWidth ? 1 : 0,
                                   (int)version) != 0;
}

@end
""")

    # Print statistics
    print(f"// Statistics:", file=sys.stderr)
    print(f"//   Stage 1 size: {len(stage1) * 2} bytes ({len(stage1)} entries)", file=sys.stderr)
    print(f"//   Stage 2 size: {len(stage2)} bytes", file=sys.stderr)
    print(f"//   Total: {len(stage1) * 2 + len(stage2)} bytes", file=sys.stderr)

if __name__ == "__main__":
    main()
