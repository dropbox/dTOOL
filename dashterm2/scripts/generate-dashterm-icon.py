#!/usr/bin/env python3
"""
Generate DashTerm2 macOS app icon with Dropbox branding.

Creates assets for Apple's Icon Composer format (.icon) as well as
traditional .iconset/.icns formats.

Dropbox Brand Colors:
- Dropbox Blue: #0061FF (RGB: 0, 97, 255)
- Display P3: 0.0, 0.38, 1.0
"""

import json
import math
import os
import shutil
import subprocess
from pathlib import Path

try:
    from PIL import Image, ImageDraw, ImageFilter, ImageFont
except ImportError:
    print("Installing Pillow...")
    subprocess.run(["pip3", "install", "Pillow"], check=True)
    from PIL import Image, ImageDraw, ImageFilter, ImageFont

# Dropbox brand colors
DROPBOX_BLUE = "#0061FF"
DROPBOX_BLUE_RGB = (0, 97, 255)
DROPBOX_BLUE_DARK = "#0047B3"
DROPBOX_BLUE_DARK_RGB = (0, 71, 179)
DROPBOX_BLUE_LIGHT = "#4D94FF"
DROPBOX_BLUE_LIGHT_RGB = (77, 148, 255)
WHITE = (255, 255, 255)
WHITE_RGBA = (255, 255, 255, 255)
DARK_BG = (20, 24, 33)

# Display P3 color values for icon.json
# Dropbox Blue in Display P3: approximately (0.0, 0.38, 1.0)
DROPBOX_BLUE_P3 = "display-p3:0.00000,0.38039,1.00000,1.00000"
DROPBOX_BLUE_DARK_P3 = "display-p3:0.00000,0.27843,0.70196,1.00000"


def draw_dropbox_logo(draw, center_x, center_y, size, color=WHITE):
    """
    Draw the Dropbox open box logo.
    The logo consists of 5 diamond shapes arranged in a specific pattern.
    """
    s = size / 100.0

    # Each diamond's half-width and half-height
    dx = 18 * s
    dy = 12 * s

    # Spacing between diamond centers horizontally
    hspace = dx * 2 + 2 * s
    # Vertical spacing
    vspace = dy + 2 * s

    def draw_diamond(cx, cy):
        points = [
            (cx, cy - dy),
            (cx + dx, cy),
            (cx, cy + dy),
            (cx - dx, cy),
        ]
        draw.polygon(points, fill=color)

    # Row 1: two diamonds at top
    draw_diamond(center_x - hspace/2, center_y - vspace)
    draw_diamond(center_x + hspace/2, center_y - vspace)

    # Row 2: two diamonds in middle (offset down)
    draw_diamond(center_x - hspace/2, center_y + dy/2)
    draw_diamond(center_x + hspace/2, center_y + dy/2)

    # Row 3: one diamond at bottom center
    draw_diamond(center_x, center_y + vspace + dy/2)


def create_bezel_dropbox_blue(size=1024):
    """
    Create a Dropbox blue bezel (rounded rectangle border).
    Similar to the existing bezel.png but in Dropbox blue.
    """
    img = Image.new('RGBA', (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # The bezel is a thick rounded rectangle border
    border_width = int(size * 0.06)  # ~6% of size
    radius = int(size * 0.22)  # macOS Big Sur style corners

    # Draw outer rounded rectangle
    draw.rounded_rectangle(
        [(0, 0), (size - 1, size - 1)],
        radius=radius,
        fill=DROPBOX_BLUE_RGB
    )

    # Draw inner rounded rectangle (to create border effect)
    inner_radius = max(0, radius - border_width)
    draw.rounded_rectangle(
        [(border_width, border_width),
         (size - 1 - border_width, size - 1 - border_width)],
        radius=inner_radius,
        fill=(0, 0, 0, 0)  # Transparent
    )

    return img


def create_dropbox_logo_asset(size=1024):
    """
    Create a standalone Dropbox logo asset for layering.
    """
    img = Image.new('RGBA', (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Draw the Dropbox logo centered
    logo_size = size * 0.6
    draw_dropbox_logo(draw, size // 2, size // 2, logo_size, WHITE)

    return img


def create_cursor_dropbox_blue(size=1024):
    """
    Create a terminal cursor in Dropbox blue.
    """
    img = Image.new('RGBA', (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Cursor dimensions (similar to existing cursor.png proportions)
    cursor_width = int(size * 0.04)
    cursor_height = int(size * 0.15)
    cursor_x = int(size * 0.48)  # Slightly left of center
    cursor_y = int(size * 0.42)  # Upper portion

    draw.rectangle(
        [(cursor_x, cursor_y),
         (cursor_x + cursor_width, cursor_y + cursor_height)],
        fill=DROPBOX_BLUE_RGB
    )

    return img


def create_cursor_white(size=1024):
    """
    Create a white terminal cursor.
    """
    img = Image.new('RGBA', (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    cursor_width = int(size * 0.04)
    cursor_height = int(size * 0.15)
    cursor_x = int(size * 0.48)
    cursor_y = int(size * 0.42)

    draw.rectangle(
        [(cursor_x, cursor_y),
         (cursor_x + cursor_width, cursor_y + cursor_height)],
        fill=WHITE
    )

    return img


def create_prompt_dropbox(size=1024):
    """
    Create a ">" prompt symbol in Dropbox blue.
    """
    img = Image.new('RGBA', (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Prompt ">" as a triangle
    prompt_size = int(size * 0.08)
    prompt_x = int(size * 0.35)
    prompt_y = int(size * 0.5)

    draw.polygon([
        (prompt_x, prompt_y - prompt_size),
        (prompt_x + prompt_size * 1.2, prompt_y),
        (prompt_x, prompt_y + prompt_size),
    ], fill=DROPBOX_BLUE_RGB)

    return img


def create_full_icon_composite(size=1024):
    """
    Create a complete composited icon (for .iconset/.icns).
    Terminal window with Dropbox branding.
    """
    img = Image.new('RGBA', (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    radius = int(size * 0.22)
    padding = int(size * 0.06)

    # Outer rounded rectangle (Dropbox blue border/frame)
    draw.rounded_rectangle(
        [(0, 0), (size - 1, size - 1)],
        radius=radius,
        fill=DROPBOX_BLUE_RGB
    )

    # Inner terminal window (dark background)
    inner_radius = int(radius * 0.7)
    draw.rounded_rectangle(
        [(padding, padding), (size - 1 - padding, size - 1 - padding)],
        radius=inner_radius,
        fill=DARK_BG
    )

    # Title bar area with window controls
    title_bar_height = int(size * 0.12)
    title_y = padding + title_bar_height

    # Window control dots
    dot_radius = int(size * 0.02)
    dot_y = padding + title_bar_height // 2
    dot_spacing = int(size * 0.05)
    dot_start_x = padding + int(size * 0.06)

    colors = [(255, 95, 86), (255, 189, 46), (39, 201, 63)]
    for i, color in enumerate(colors):
        cx = dot_start_x + i * dot_spacing
        draw.ellipse(
            [(cx - dot_radius, dot_y - dot_radius),
             (cx + dot_radius, dot_y + dot_radius)],
            fill=color
        )

    # Separator line
    draw.line(
        [(padding, title_y), (size - padding, title_y)],
        fill=(60, 65, 80),
        width=1
    )

    # Dropbox logo in terminal content area
    content_center_y = title_y + (size - padding - title_y) // 2
    logo_size = size * 0.4
    draw_dropbox_logo(
        draw,
        size // 2,
        int(content_center_y - size * 0.05),
        logo_size,
        DROPBOX_BLUE_LIGHT_RGB
    )

    # Terminal prompt below logo
    prompt_y = int(content_center_y + size * 0.18)
    prompt_x = padding + int(size * 0.08)
    prompt_size = int(size * 0.05)

    draw.polygon([
        (prompt_x, prompt_y - prompt_size),
        (prompt_x + prompt_size, prompt_y),
        (prompt_x, prompt_y + prompt_size),
    ], fill=DROPBOX_BLUE_RGB)

    # Block cursor
    cursor_x = prompt_x + prompt_size + int(size * 0.04)
    cursor_width = int(size * 0.045)
    cursor_height = int(size * 0.09)
    draw.rectangle(
        [(cursor_x, prompt_y - cursor_height // 2),
         (cursor_x + cursor_width, prompt_y + cursor_height // 2)],
        fill=WHITE
    )

    return img


def create_icon_json_dropbox():
    """
    Create icon.json for the .icon package with Dropbox branding.
    Uses Dropbox blue gradient background.
    """
    return {
        "color-space-for-untagged-svg-colors": "display-p3",
        "fill": {
            "linear-gradient": [
                DROPBOX_BLUE_P3,
                DROPBOX_BLUE_DARK_P3
            ]
        },
        "groups": [
            {
                "layers": [
                    {
                        "image-name": "dropbox-logo.png",
                        "name": "dropbox-logo",
                        "position": {
                            "scale": 0.5,
                            "translation-in-points": [
                                256,
                                200
                            ]
                        }
                    },
                    {
                        "image-name": "cursor-white.png",
                        "name": "cursor",
                        "position": {
                            "scale": 1,
                            "translation-in-points": [
                                150,
                                350
                            ]
                        }
                    },
                    {
                        "image-name": "bezel-dropbox.png",
                        "name": "bezel",
                        "position": {
                            "scale": 0.67,
                            "translation-in-points": [
                                0,
                                0
                            ]
                        }
                    }
                ],
                "shadow": {
                    "kind": "neutral",
                    "opacity": 0.5
                },
                "translucency": {
                    "enabled": True,
                    "value": 0.3
                }
            }
        ],
        "supported-platforms": {
            "squares": [
                "macOS"
            ]
        }
    }


def main():
    """Generate all icon assets and packages."""
    script_dir = Path(__file__).parent.parent
    images_dir = script_dir / "images"
    appicon_dir = images_dir / "AppIcon"

    # ========================================
    # Part 1: Create .iconset and .icns (traditional format)
    # ========================================
    iconset_dir = images_dir / "DashTerm2.iconset"
    iconset_dir.mkdir(exist_ok=True)

    icon_specs = [
        (16, 1, "icon_16x16.png"),
        (16, 2, "icon_16x16@2x.png"),
        (32, 1, "icon_32x32.png"),
        (32, 2, "icon_32x32@2x.png"),
        (128, 1, "icon_128x128.png"),
        (128, 2, "icon_128x128@2x.png"),
        (256, 1, "icon_256x256.png"),
        (256, 2, "icon_256x256@2x.png"),
        (512, 1, "icon_512x512.png"),
        (512, 2, "icon_512x512@2x.png"),
    ]

    print("=" * 60)
    print("Generating DashTerm2 icon with Dropbox branding")
    print(f"Dropbox Blue: {DROPBOX_BLUE}")
    print("=" * 60)
    print()

    # Generate master icon
    master_size = 1024
    master_icon = create_full_icon_composite(master_size)

    print("[1/4] Creating .iconset (traditional format)...")
    for base_size, scale, filename in icon_specs:
        actual_size = base_size * scale
        icon = master_icon.resize((actual_size, actual_size), Image.LANCZOS)
        output_path = iconset_dir / filename
        icon.save(output_path, 'PNG')
        print(f"       {filename} ({actual_size}x{actual_size})")

    # Save master for reference
    master_icon.save(images_dir / "DashTerm2-AppIcon.png", 'PNG')
    print(f"       DashTerm2-AppIcon.png (1024x1024)")

    # Create .icns
    icns_path = images_dir / "DashTerm2.icns"
    try:
        subprocess.run(
            ["iconutil", "-c", "icns", str(iconset_dir), "-o", str(icns_path)],
            capture_output=True, text=True, check=True
        )
        print(f"       DashTerm2.icns (created)")
    except subprocess.CalledProcessError as e:
        print(f"       Error creating .icns: {e.stderr}")
    except FileNotFoundError:
        print("       iconutil not found (not on macOS)")

    # ========================================
    # Part 2: Create .icon package assets (Icon Composer format)
    # ========================================
    print()
    print("[2/4] Creating Icon Composer assets...")

    # Create individual assets at 1024x1024
    asset_size = 1024

    # Dropbox logo
    logo_img = create_dropbox_logo_asset(asset_size)
    logo_path = appicon_dir / "dropbox-logo.png"
    logo_img.save(logo_path, 'PNG')
    print(f"       dropbox-logo.png")

    # Bezel in Dropbox blue
    bezel_img = create_bezel_dropbox_blue(asset_size)
    bezel_path = appicon_dir / "bezel-dropbox.png"
    bezel_img.save(bezel_path, 'PNG')
    print(f"       bezel-dropbox.png")

    # White cursor
    cursor_img = create_cursor_white(asset_size)
    cursor_path = appicon_dir / "cursor-white.png"
    cursor_img.save(cursor_path, 'PNG')
    print(f"       cursor-white.png")

    # Dropbox blue cursor
    cursor_blue_img = create_cursor_dropbox_blue(asset_size)
    cursor_blue_path = appicon_dir / "cursor-dropbox.png"
    cursor_blue_img.save(cursor_blue_path, 'PNG')
    print(f"       cursor-dropbox.png")

    # ========================================
    # Part 3: Create .icon package
    # ========================================
    print()
    print("[3/4] Creating .icon package (Icon Composer format)...")

    icon_pkg_name = "DashTerm2 App Icon for Dropbox.icon"
    icon_pkg_dir = appicon_dir / icon_pkg_name
    icon_pkg_assets = icon_pkg_dir / "Assets"

    # Create directory structure
    icon_pkg_dir.mkdir(exist_ok=True)
    icon_pkg_assets.mkdir(exist_ok=True)

    # Copy assets into package
    shutil.copy(logo_path, icon_pkg_assets / "dropbox-logo.png")
    shutil.copy(bezel_path, icon_pkg_assets / "bezel-dropbox.png")
    shutil.copy(cursor_path, icon_pkg_assets / "cursor-white.png")

    # Write icon.json
    icon_json = create_icon_json_dropbox()
    icon_json_path = icon_pkg_dir / "icon.json"
    with open(icon_json_path, 'w') as f:
        json.dump(icon_json, f, indent=2)
    print(f"       {icon_pkg_name}/icon.json")
    print(f"       {icon_pkg_name}/Assets/dropbox-logo.png")
    print(f"       {icon_pkg_name}/Assets/bezel-dropbox.png")
    print(f"       {icon_pkg_name}/Assets/cursor-white.png")

    # ========================================
    # Part 4: Update existing .icon packages with Dropbox branding
    # ========================================
    print()
    print("[4/4] Updating existing .icon packages...")

    for variant in ["Release", "Beta", "Nightly"]:
        variant_dir = appicon_dir / f"DashTerm2 App Icon for {variant}.icon"
        if variant_dir.exists():
            variant_assets = variant_dir / "Assets"
            variant_assets.mkdir(exist_ok=True)

            # Copy Dropbox assets
            shutil.copy(logo_path, variant_assets / "dropbox-logo.png")
            shutil.copy(bezel_path, variant_assets / "bezel-dropbox.png")
            shutil.copy(cursor_path, variant_assets / "cursor-white.png")

            # Update icon.json
            icon_json_variant = create_icon_json_dropbox()
            with open(variant_dir / "icon.json", 'w') as f:
                json.dump(icon_json_variant, f, indent=2)

            print(f"       Updated: DashTerm2 App Icon for {variant}.icon")

    print()
    print("=" * 60)
    print("Icon generation complete!")
    print()
    print("Created files:")
    print(f"  - {iconset_dir}")
    print(f"  - {icns_path}")
    print(f"  - {images_dir / 'DashTerm2-AppIcon.png'}")
    print(f"  - {icon_pkg_dir}")
    print()
    print("Updated .icon packages for Release, Beta, and Nightly builds")
    print("=" * 60)

    return 0


if __name__ == "__main__":
    exit(main())
