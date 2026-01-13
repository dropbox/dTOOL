#!/usr/bin/env python3
"""
Generate ANSI color escape sequences for benchmarking terminal rendering.

Usage:
    python3 generate_color_test.py [--mode MODE] [--lines LINES]

Modes:
    256     - 256-color mode using ESC[38;5;Nm format (default)
    24bit   - 24-bit color mode using ESC[38;2;R;G;Bm format
    mixed   - Mix of 256-color and 24-bit colors
"""

import argparse
import sys

def generate_256_color_line():
    """Generate a line with all 256 foreground colors."""
    parts = []
    for c in range(256):
        parts.append(f'\033[38;5;{c}m#')
    parts.append('\033[0m\n')  # Reset at end of line
    return ''.join(parts)

def generate_24bit_color_line():
    """Generate a line with 256 different 24-bit colors."""
    parts = []
    for i in range(256):
        r = i
        g = (i * 3) % 256
        b = (i * 7) % 256
        parts.append(f'\033[38;2;{r};{g};{b}m#')
    parts.append('\033[0m\n')
    return ''.join(parts)

def generate_mixed_color_line():
    """Generate a line mixing 256-color and 24-bit colors."""
    parts = []
    for i in range(256):
        if i % 2 == 0:
            parts.append(f'\033[38;5;{i}m#')
        else:
            r = i
            g = (i * 3) % 256
            b = (i * 7) % 256
            parts.append(f'\033[38;2;{r};{g};{b}m#')
    parts.append('\033[0m\n')
    return ''.join(parts)

def main():
    parser = argparse.ArgumentParser(description='Generate ANSI color escape sequences')
    parser.add_argument('--mode', choices=['256', '24bit', 'mixed'], default='256',
                        help='Color mode (default: 256)')
    parser.add_argument('--lines', type=int, default=11,
                        help='Number of lines to generate (default: 11)')
    parser.add_argument('--file', type=str, default=None,
                        help='Output to file instead of stdout')
    args = parser.parse_args()

    generators = {
        '256': generate_256_color_line,
        '24bit': generate_24bit_color_line,
        'mixed': generate_mixed_color_line
    }

    generator = generators[args.mode]
    line = generator()

    output = sys.stdout
    if args.file:
        output = open(args.file, 'w')

    for _ in range(args.lines):
        output.write(line)

    if args.file:
        output.close()

if __name__ == '__main__':
    main()
