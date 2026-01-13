#!/usr/bin/env python3
"""
Capture DashTerm2 window screenshots for AI visual debugging.

Usage:
    ./capture-dashterm.py                    # Capture to timestamped file
    ./capture-dashterm.py --output file.png  # Capture to specific file
    ./capture-dashterm.py --type-text "ls"   # Type text first, then capture
"""

import Quartz
import subprocess
import sys
import time
import argparse
from datetime import datetime


def get_dashterm_windows():
    """Find all DashTerm2 windows on screen."""
    windows = Quartz.CGWindowListCopyWindowInfo(
        Quartz.kCGWindowListOptionOnScreenOnly,
        Quartz.kCGNullWindowID
    )

    dashterm_windows = []
    for win in windows:
        owner = win.get('kCGWindowOwnerName', '')
        if 'DashTerm' not in owner:
            continue

        name = win.get('kCGWindowName', '') or '(no name)'
        wid = win.get('kCGWindowNumber', 0)
        bounds = win.get('kCGWindowBounds', {})
        layer = win.get('kCGWindowLayer', 0)
        w = bounds.get('Width', 0)
        h = bounds.get('Height', 0)
        x = bounds.get('X', 0)
        y = bounds.get('Y', 0)

        dashterm_windows.append({
            'wid': wid,
            'name': name,
            'width': w,
            'height': h,
            'x': x,
            'y': y,
            'layer': layer,
            'area': w * h
        })

    return dashterm_windows


def capture_window(window_id, output_path):
    """Capture a window by ID."""
    result = subprocess.run(
        ['screencapture', '-l', str(window_id), '-x', output_path],
        capture_output=True
    )
    return result.returncode == 0


def type_to_terminal(text, press_enter=True):
    """Send keystrokes to DashTerm2."""
    # Escape special characters for AppleScript
    escaped = text.replace('\\', '\\\\').replace('"', '\\"')

    script = f'''
    tell application "DashTerm2" to activate
    delay 0.2
    tell application "System Events"
        keystroke "{escaped}"
    end tell
    '''

    if press_enter:
        script += '''
    delay 0.1
    tell application "System Events"
        keystroke return
    end tell
    '''

    subprocess.run(['osascript', '-e', script], capture_output=True)


def main():
    parser = argparse.ArgumentParser(description='Capture DashTerm2 window')
    parser.add_argument('--output', '-o', help='Output file path')
    parser.add_argument('--type-text', '-t', help='Text to type before capture')
    parser.add_argument('--no-enter', action='store_true', help="Don't press enter after typing")
    parser.add_argument('--delay', '-d', type=float, default=0.5, help='Delay after typing before capture')
    parser.add_argument('--list', '-l', action='store_true', help='List DashTerm2 windows')
    parser.add_argument('--wid', type=int, help='Specific window ID to capture')
    args = parser.parse_args()

    # List windows if requested
    if args.list:
        windows = get_dashterm_windows()
        if not windows:
            print("No DashTerm2 windows found")
            return 1
        for w in windows:
            print(f"WID:{w['wid']} | {w['width']:.0f}x{w['height']:.0f} | {w['name']}")
        return 0

    # Find DashTerm2 windows
    windows = get_dashterm_windows()
    if not windows:
        print("ERROR: No DashTerm2 windows found on screen", file=sys.stderr)
        return 1

    # Select window
    if args.wid:
        target = next((w for w in windows if w['wid'] == args.wid), None)
        if not target:
            print(f"ERROR: Window ID {args.wid} not found", file=sys.stderr)
            return 1
    else:
        # Find the largest window (likely the main terminal)
        target = max(windows, key=lambda w: w['area'])

    print(f"Target window: WID:{target['wid']} ({target['width']:.0f}x{target['height']:.0f}) '{target['name']}'")

    # Type text if requested
    if args.type_text:
        print(f"Typing: {args.type_text}")
        type_to_terminal(args.type_text, press_enter=not args.no_enter)
        time.sleep(args.delay)

    # Generate output path
    if args.output:
        output_path = args.output
    else:
        timestamp = datetime.now().strftime('%Y%m%d_%H%M%S')
        output_path = f'/tmp/dashterm_{timestamp}.png'

    # Capture
    if capture_window(target['wid'], output_path):
        print(f"Captured: {output_path}")
        return 0
    else:
        print("ERROR: Capture failed", file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
