#!/usr/bin/env python3
"""
Generate DashTerm2 menu bar icon - clean Dropbox logo.
"""

import subprocess
from pathlib import Path

try:
    from PIL import Image, ImageDraw
except ImportError:
    subprocess.run(["pip3", "install", "Pillow"], check=True)
    from PIL import Image, ImageDraw

# Dropbox Blue
DROPBOX_BLUE = (0, 97, 255, 255)


def draw_dropbox_logo(draw, cx, cy, size, color):
    """Draw the Dropbox box logo."""
    s = size / 100.0
    dx = 22 * s
    dy = 14 * s
    hspace = dx * 2 + 2 * s
    vspace = dy + 2 * s

    def diamond(x, y):
        draw.polygon([(x, y - dy), (x + dx, y), (x, y + dy), (x - dx, y)], fill=color)

    diamond(cx - hspace/2, cy - vspace * 0.5)
    diamond(cx + hspace/2, cy - vspace * 0.5)
    diamond(cx - hspace/2, cy + dy * 0.5)
    diamond(cx + hspace/2, cy + dy * 0.5)
    diamond(cx, cy + vspace * 0.5 + dy * 0.5)


def main():
    images_dir = Path(__file__).parent.parent / "images"

    for size, name in [(18, "StatusItem.png"), (36, "StatusItem@2x.png"),
                       (22, "StatusItem-22.png"), (44, "StatusItem-22@2x.png")]:
        img = Image.new('RGBA', (size, size), (0, 0, 0, 0))
        draw = ImageDraw.Draw(img)
        # Just the Dropbox logo in Dropbox Blue - no background
        draw_dropbox_logo(draw, size/2, size/2, size * 0.95, DROPBOX_BLUE)
        img.save(images_dir / name, 'PNG')
        print(f"Created: {name}")


if __name__ == "__main__":
    main()
