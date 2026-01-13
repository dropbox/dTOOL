#!/usr/bin/env python3
"""
DashTerm2 Visual Comparison Test with LLM-as-Judge

This integration test:
1. Opens new windows in both iTerm2 and DashTerm2
2. Sends identical commands to both terminals
3. Takes screenshots of both windows
4. Creates a heatmap diff visualization
5. Sends all three images to LLM judges (Opus 4.5 and/or GPT-4o) for analysis

Usage:
    python3 llm_visual_judge.py [--test-case NAME] [--all] [--opus-only] [--gpt-only]

Environment Variables:
    ANTHROPIC_API_KEY - Required for Opus 4.5 evaluation
    OPENAI_API_KEY    - Required for GPT evaluation
    ANTHROPIC_MODEL   - Override Anthropic model (default: claude-opus-4-5-20250514)
    OPENAI_MODEL      - Override OpenAI model (default: gpt-5.2)

Requirements:
    pip3 install Pillow numpy anthropic openai

Author: DashTerm2 Visual QA System
"""

import argparse
import base64
import json
import os
import subprocess
import sys
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime
from io import BytesIO
from pathlib import Path
from typing import Dict, List, Optional, Tuple

try:
    from PIL import Image, ImageDraw, ImageFont
    import numpy as np
except ImportError:
    print("Missing dependencies. Install with: pip3 install Pillow numpy")
    sys.exit(1)

# =============================================================================
# Configuration
# =============================================================================

PROJECT_ROOT = Path(__file__).parent.parent
OUTPUT_DIR = PROJECT_ROOT / "visual-test-output" / "llm-judge"
DASHTERM_APP_PATTERN = "~/Library/Developer/Xcode/DerivedData/DashTerm2-*/Build/Products/Development/DashTerm2.app"

# LLM Models - can be overridden via environment variables
ANTHROPIC_MODEL = os.environ.get("ANTHROPIC_MODEL", "claude-opus-4-5-20250514")  # Opus 4.5
OPENAI_MODEL = os.environ.get("OPENAI_MODEL", "gpt-5.2")  # GPT-5.2

# Test cases: (name, description, command, wait_seconds)
TEST_CASES = [
    (
        "basic_text",
        "Basic text rendering - tests font clarity and spacing",
        "echo 'Hello, World! The quick brown fox jumps over the lazy dog. 0123456789'",
        2
    ),
    (
        "box_drawing_single",
        "Single-line box drawing characters - tests Unicode rendering",
        r"printf '\n┌───────────────────────────┐\n│   Single Line Box Test    │\n│   ├── Nested Item         │\n│   └── Another Item        │\n└───────────────────────────┘\n'",
        2
    ),
    (
        "box_drawing_double",
        "Double-line box drawing characters",
        r"printf '\n╔═══════════════════════════╗\n║   Double Line Box Test    ║\n║   ╠══ Nested Item         ║\n║   ╚══ Another Item        ║\n╚═══════════════════════════╝\n'",
        2
    ),
    (
        "ansi_colors_basic",
        "Basic ANSI color rendering - 8 standard colors",
        r"printf '\e[30m Black \e[31m Red \e[32m Green \e[33m Yellow \e[0m\n\e[34m Blue \e[35m Magenta \e[36m Cyan \e[37m White \e[0m\n'",
        2
    ),
    (
        "ansi_colors_256",
        "256-color palette test",
        r"for i in {0..15}; do printf '\e[48;5;%dm  \e[0m' $i; done; echo",
        2
    ),
    (
        "text_decorations",
        "Text decorations - bold, italic, underline, strikethrough",
        r"printf '\e[1mBold\e[0m \e[3mItalic\e[0m \e[4mUnderline\e[0m \e[9mStrikethrough\e[0m \e[1;3;4mAll Three\e[0m\n'",
        2
    ),
    (
        "unicode_symbols",
        "Unicode symbols and special characters",
        "echo 'Arrows: -> <- <-> => Bullets: * - + Math: +- x / = != <= >='",
        2
    ),
    (
        "unicode_emoji",
        "Emoji and wide characters",
        "echo 'Emoji: [check] [x] [star] [heart] Flags: [US] [JP]'",
        2
    ),
    (
        "dense_listing",
        "Dense text - file listing for smear/blur detection",
        "ls -la /usr/bin | head -20",
        3
    ),
    (
        "cursor_position",
        "Cursor positioning and prompt rendering",
        "clear && echo 'Line 1' && echo 'Line 2' && echo 'Line 3'",
        2
    ),
]


# =============================================================================
# Terminal Window Management
# =============================================================================

def get_window_id(app_name: str) -> Optional[int]:
    """Get CGWindowID for an application's main window."""
    script = f'''
import Quartz
for w in Quartz.CGWindowListCopyWindowInfo(Quartz.kCGWindowListOptionAll, Quartz.kCGNullWindowID):
    owner = w.get('kCGWindowOwnerName', '')
    name = w.get('kCGWindowName', '')
    layer = w.get('kCGWindowLayer', 0)
    if '{app_name}' in owner and name and layer == 0:
        print(w.get('kCGWindowNumber'))
        break
'''
    result = subprocess.run(['python3', '-c', script], capture_output=True, text=True)
    if result.stdout.strip():
        return int(result.stdout.strip())
    return None


def get_all_window_ids(app_name: str) -> List[Tuple[int, str]]:
    """Get all window IDs for an application."""
    script = f'''
import Quartz
results = []
for w in Quartz.CGWindowListCopyWindowInfo(Quartz.kCGWindowListOptionAll, Quartz.kCGNullWindowID):
    owner = w.get('kCGWindowOwnerName', '')
    name = w.get('kCGWindowName', '')
    layer = w.get('kCGWindowLayer', 0)
    if '{app_name}' in owner and layer == 0:
        print(f"{{w.get('kCGWindowNumber')}}|{{name}}")
'''
    result = subprocess.run(['python3', '-c', script], capture_output=True, text=True)
    windows = []
    for line in result.stdout.strip().split('\n'):
        if '|' in line:
            wid, name = line.split('|', 1)
            windows.append((int(wid), name))
    return windows


def capture_window(window_id: int, output_path: Path) -> bool:
    """Capture a window screenshot."""
    result = subprocess.run(
        ['screencapture', '-l', str(window_id), '-x', str(output_path)],
        capture_output=True
    )
    return result.returncode == 0 and output_path.exists()


def send_command(app_name: str, command: str) -> bool:
    """Send a command to a terminal application via AppleScript."""
    # Escape quotes and backslashes for AppleScript
    escaped_cmd = command.replace('\\', '\\\\').replace('"', '\\"')
    script = f'''
tell application "{app_name}"
    activate
    delay 0.3
    tell current session of current window
        write text "{escaped_cmd}"
    end tell
end tell
'''
    result = subprocess.run(['osascript', '-e', script], capture_output=True)
    return result.returncode == 0


def new_window(app_name: str) -> bool:
    """Open a new window in the terminal application."""
    script = f'''
tell application "{app_name}"
    activate
    delay 0.2
    tell application "System Events"
        keystroke "n" using command down
    end tell
end tell
'''
    result = subprocess.run(['osascript', '-e', script], capture_output=True)
    return result.returncode == 0


def close_all_windows(app_name: str) -> bool:
    """Close all windows of an application."""
    script = f'''
tell application "{app_name}"
    close every window
end tell
'''
    subprocess.run(['osascript', '-e', script], capture_output=True)
    return True


# =============================================================================
# Image Processing and Heatmap Generation
# =============================================================================

def image_to_base64(img: Image.Image, format: str = "PNG") -> str:
    """Convert PIL Image to base64 string."""
    buffer = BytesIO()
    img.save(buffer, format=format)
    return base64.b64encode(buffer.getvalue()).decode('utf-8')


def load_image_as_base64(path: Path) -> str:
    """Load an image file and return as base64."""
    with open(path, 'rb') as f:
        return base64.b64encode(f.read()).decode('utf-8')


def extract_terminal_content(img: Image.Image, margin_top: int = 70, margin_bottom: int = 20) -> Image.Image:
    """
    Extract just the terminal content area, removing title bar and decorations.
    """
    width, height = img.size
    return img.crop((0, margin_top, width, height - margin_bottom))


def create_heatmap_diff(img1: Image.Image, img2: Image.Image) -> Tuple[Image.Image, float, Dict]:
    """
    Create a heatmap visualization of differences between two images.

    Returns:
        - heatmap_image: Visual heatmap (red = different, green = same)
        - similarity_score: Float 0-100 (100 = identical)
        - stats: Dictionary with detailed statistics
    """
    # Ensure same size by cropping to minimum dimensions
    min_w = min(img1.size[0], img2.size[0])
    min_h = min(img1.size[1], img2.size[1])
    img1 = img1.crop((0, 0, min_w, min_h))
    img2 = img2.crop((0, 0, min_w, min_h))

    # Convert to numpy arrays
    arr1 = np.array(img1.convert('RGB'), dtype=np.float32)
    arr2 = np.array(img2.convert('RGB'), dtype=np.float32)

    # Compute per-pixel difference using Euclidean distance in RGB space
    diff = np.abs(arr1 - arr2)
    diff_magnitude = np.sqrt(np.sum(diff ** 2, axis=2))

    # Normalize (max possible diff is sqrt(3 * 255^2) = ~441.67)
    max_diff = np.sqrt(3 * 255**2)
    diff_normalized = diff_magnitude / max_diff

    # Create heatmap visualization
    # Use a red-yellow-green color scheme
    heatmap = np.zeros((*diff_normalized.shape, 3), dtype=np.uint8)

    # Threshold levels
    low_thresh = 0.05   # < 5% difference = green (similar)
    mid_thresh = 0.15   # 5-15% = yellow (minor difference)
    high_thresh = 0.30  # 15-30% = orange (moderate difference)
    # > 30% = red (significant difference)

    # Green channel (high for similar pixels)
    heatmap[:, :, 1] = np.where(diff_normalized < low_thresh, 200,
                       np.where(diff_normalized < mid_thresh, 150,
                       np.where(diff_normalized < high_thresh, 80, 0))).astype(np.uint8)

    # Red channel (high for different pixels)
    heatmap[:, :, 0] = np.where(diff_normalized < low_thresh, 0,
                       np.where(diff_normalized < mid_thresh, 150,
                       np.where(diff_normalized < high_thresh, 200, 255))).astype(np.uint8)

    # Blue channel (for very different = purple tinge)
    heatmap[:, :, 2] = np.where(diff_normalized > high_thresh, 100, 0).astype(np.uint8)

    # Overlay on grayscale version of original for context
    gray1 = np.array(img1.convert('L'))
    alpha = 0.6  # Blend factor

    for c in range(3):
        heatmap[:, :, c] = (alpha * heatmap[:, :, c] + (1 - alpha) * gray1).astype(np.uint8)

    heatmap_image = Image.fromarray(heatmap)

    # Calculate statistics
    total_pixels = diff_normalized.size
    similar_pixels = np.sum(diff_normalized < low_thresh)
    minor_diff_pixels = np.sum((diff_normalized >= low_thresh) & (diff_normalized < mid_thresh))
    moderate_diff_pixels = np.sum((diff_normalized >= mid_thresh) & (diff_normalized < high_thresh))
    major_diff_pixels = np.sum(diff_normalized >= high_thresh)

    similarity_score = (similar_pixels / total_pixels) * 100

    stats = {
        'total_pixels': int(total_pixels),
        'similar_pixels': int(similar_pixels),
        'minor_diff_pixels': int(minor_diff_pixels),
        'moderate_diff_pixels': int(moderate_diff_pixels),
        'major_diff_pixels': int(major_diff_pixels),
        'similarity_percent': round(similarity_score, 2),
        'mean_diff': round(float(np.mean(diff_normalized) * 100), 2),
        'max_diff': round(float(np.max(diff_normalized) * 100), 2),
        'std_diff': round(float(np.std(diff_normalized) * 100), 2),
    }

    return heatmap_image, similarity_score, stats


def create_comparison_panel(
    dashterm_img: Image.Image,
    iterm_img: Image.Image,
    heatmap_img: Image.Image,
    test_name: str,
    similarity: float
) -> Image.Image:
    """Create a side-by-side comparison panel with all three images."""
    # Ensure all images are same height
    max_h = max(dashterm_img.size[1], iterm_img.size[1], heatmap_img.size[1])

    def pad_to_height(img, target_h):
        if img.size[1] < target_h:
            new_img = Image.new('RGB', (img.size[0], target_h), (40, 40, 40))
            new_img.paste(img, (0, 0))
            return new_img
        return img

    dashterm_img = pad_to_height(dashterm_img, max_h)
    iterm_img = pad_to_height(iterm_img, max_h)
    heatmap_img = pad_to_height(heatmap_img, max_h)

    # Layout
    padding = 15
    header_height = 50
    total_width = dashterm_img.size[0] + iterm_img.size[0] + heatmap_img.size[0] + padding * 4
    total_height = max_h + header_height + padding * 2

    panel = Image.new('RGB', (total_width, total_height), (25, 25, 25))
    draw = ImageDraw.Draw(panel)

    # Try to load a font
    try:
        title_font = ImageFont.truetype("/System/Library/Fonts/Helvetica.ttc", 18)
        label_font = ImageFont.truetype("/System/Library/Fonts/Helvetica.ttc", 14)
    except:
        title_font = ImageFont.load_default()
        label_font = title_font

    # Title
    title = f"Visual Comparison: {test_name}"
    draw.text((padding, padding // 2), title, fill=(200, 200, 200), font=title_font)

    # Similarity badge
    sim_color = (100, 255, 100) if similarity > 95 else (255, 255, 100) if similarity > 80 else (255, 100, 100)
    sim_text = f"{similarity:.1f}% Similar"
    draw.text((total_width - 150, padding // 2), sim_text, fill=sim_color, font=title_font)

    y_offset = header_height
    x_offset = padding

    # DashTerm2 image
    draw.text((x_offset, y_offset - 20), "DashTerm2 (Test)", fill=(100, 180, 255), font=label_font)
    panel.paste(dashterm_img, (x_offset, y_offset))
    x_offset += dashterm_img.size[0] + padding

    # iTerm2 image
    draw.text((x_offset, y_offset - 20), "iTerm2 (Reference)", fill=(100, 255, 100), font=label_font)
    panel.paste(iterm_img, (x_offset, y_offset))
    x_offset += iterm_img.size[0] + padding

    # Heatmap
    draw.text((x_offset, y_offset - 20), "Difference Heatmap", fill=(255, 180, 100), font=label_font)
    panel.paste(heatmap_img, (x_offset, y_offset))

    return panel


# =============================================================================
# LLM Integration
# =============================================================================

class LLMJudge:
    """Base class for LLM visual judges."""

    def __init__(self, name: str):
        self.name = name

    def evaluate(
        self,
        dashterm_b64: str,
        iterm_b64: str,
        heatmap_b64: str,
        test_name: str,
        test_description: str,
        similarity_stats: Dict
    ) -> Dict:
        """Evaluate the visual comparison and return structured results."""
        raise NotImplementedError


class AnthropicJudge(LLMJudge):
    """Anthropic Claude visual comparison judge."""

    def __init__(self):
        super().__init__(f"Anthropic ({ANTHROPIC_MODEL})")
        self.api_key = os.environ.get('ANTHROPIC_API_KEY')
        if not self.api_key:
            raise ValueError("ANTHROPIC_API_KEY environment variable not set")

        try:
            import anthropic
            self.client = anthropic.Anthropic(api_key=self.api_key)
        except ImportError:
            raise ImportError("anthropic package not installed. Run: pip3 install anthropic")

    def evaluate(
        self,
        dashterm_b64: str,
        iterm_b64: str,
        heatmap_b64: str,
        test_name: str,
        test_description: str,
        similarity_stats: Dict
    ) -> Dict:
        """Evaluate using Claude Opus 4.5."""

        prompt = self._build_prompt(test_name, test_description, similarity_stats)

        try:
            response = self.client.messages.create(
                model=ANTHROPIC_MODEL,
                max_tokens=2000,
                messages=[{
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": prompt
                        },
                        {
                            "type": "text",
                            "text": "Image 1 - DashTerm2 (Test Subject):"
                        },
                        {
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/png",
                                "data": dashterm_b64
                            }
                        },
                        {
                            "type": "text",
                            "text": "Image 2 - iTerm2 (Reference/Ground Truth):"
                        },
                        {
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/png",
                                "data": iterm_b64
                            }
                        },
                        {
                            "type": "text",
                            "text": "Image 3 - Heatmap Diff (Red=Different, Green=Similar):"
                        },
                        {
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/png",
                                "data": heatmap_b64
                            }
                        }
                    ]
                }]
            )

            return self._parse_response(response.content[0].text)

        except Exception as e:
            return {
                "error": str(e),
                "judge": self.name,
                "verdict": "ERROR",
                "score": 0
            }

    def _build_prompt(self, test_name: str, test_description: str, stats: Dict) -> str:
        return f"""You are a visual QA expert comparing terminal emulator rendering.

TEST: {test_name}
DESCRIPTION: {test_description}

PIXEL STATISTICS:
- Similarity: {stats['similarity_percent']}%
- Mean difference: {stats['mean_diff']}%
- Max difference: {stats['max_diff']}%
- Similar pixels: {stats['similar_pixels']:,} / {stats['total_pixels']:,}
- Minor differences: {stats['minor_diff_pixels']:,}
- Moderate differences: {stats['moderate_diff_pixels']:,}
- Major differences: {stats['major_diff_pixels']:,}

You will see three images:
1. DashTerm2 screenshot (the terminal under test)
2. iTerm2 screenshot (the reference/correct rendering)
3. Heatmap showing pixel-level differences (red=different, green=similar)

Analyze the visual differences and provide a structured assessment. Focus on:
- Text clarity and sharpness (blurring, smearing)
- Character alignment and spacing
- Box drawing character rendering
- Color accuracy
- Unicode/emoji rendering
- Overall visual fidelity

Respond in this exact JSON format:
{{
    "verdict": "PASS" | "MINOR_ISSUES" | "FAIL",
    "score": <0-100 integer>,
    "summary": "<one sentence summary>",
    "issues": [
        {{
            "category": "text_clarity" | "box_drawing" | "colors" | "unicode" | "spacing" | "other",
            "severity": "minor" | "moderate" | "major",
            "description": "<specific description of the issue>"
        }}
    ],
    "details": "<2-3 sentences with specific observations>"
}}

Be precise and technical. If the rendering is identical or nearly identical, say so. If there are issues, describe them specifically."""

    def _parse_response(self, text: str) -> Dict:
        """Parse LLM response to extract JSON."""
        # Try to find JSON in the response
        import re
        json_match = re.search(r'\{[\s\S]*\}', text)
        if json_match:
            try:
                result = json.loads(json_match.group())
                result['judge'] = self.name
                result['raw_response'] = text
                return result
            except json.JSONDecodeError:
                pass

        # Fallback if JSON parsing fails
        return {
            "judge": self.name,
            "verdict": "PARSE_ERROR",
            "score": 0,
            "summary": "Failed to parse LLM response",
            "raw_response": text
        }


class OpenAIJudge(LLMJudge):
    """OpenAI GPT visual comparison judge."""

    def __init__(self):
        super().__init__(f"OpenAI ({OPENAI_MODEL})")
        self.api_key = os.environ.get('OPENAI_API_KEY')
        if not self.api_key:
            raise ValueError("OPENAI_API_KEY environment variable not set")

        try:
            import openai
            self.client = openai.OpenAI(api_key=self.api_key)
        except ImportError:
            raise ImportError("openai package not installed. Run: pip3 install openai")

    def evaluate(
        self,
        dashterm_b64: str,
        iterm_b64: str,
        heatmap_b64: str,
        test_name: str,
        test_description: str,
        similarity_stats: Dict
    ) -> Dict:
        """Evaluate using GPT-4o."""

        prompt = self._build_prompt(test_name, test_description, similarity_stats)

        try:
            response = self.client.chat.completions.create(
                model=OPENAI_MODEL,
                max_completion_tokens=2000,
                messages=[{
                    "role": "user",
                    "content": [
                        {"type": "text", "text": prompt},
                        {"type": "text", "text": "Image 1 - DashTerm2 (Test Subject):"},
                        {
                            "type": "image_url",
                            "image_url": {"url": f"data:image/png;base64,{dashterm_b64}"}
                        },
                        {"type": "text", "text": "Image 2 - iTerm2 (Reference/Ground Truth):"},
                        {
                            "type": "image_url",
                            "image_url": {"url": f"data:image/png;base64,{iterm_b64}"}
                        },
                        {"type": "text", "text": "Image 3 - Heatmap Diff (Red=Different, Green=Similar):"},
                        {
                            "type": "image_url",
                            "image_url": {"url": f"data:image/png;base64,{heatmap_b64}"}
                        }
                    ]
                }]
            )

            return self._parse_response(response.choices[0].message.content)

        except Exception as e:
            return {
                "error": str(e),
                "judge": self.name,
                "verdict": "ERROR",
                "score": 0
            }

    def _build_prompt(self, test_name: str, test_description: str, stats: Dict) -> str:
        # Same prompt as Anthropic for consistency
        return f"""You are a visual QA expert comparing terminal emulator rendering.

TEST: {test_name}
DESCRIPTION: {test_description}

PIXEL STATISTICS:
- Similarity: {stats['similarity_percent']}%
- Mean difference: {stats['mean_diff']}%
- Max difference: {stats['max_diff']}%
- Similar pixels: {stats['similar_pixels']:,} / {stats['total_pixels']:,}
- Minor differences: {stats['minor_diff_pixels']:,}
- Moderate differences: {stats['moderate_diff_pixels']:,}
- Major differences: {stats['major_diff_pixels']:,}

You will see three images:
1. DashTerm2 screenshot (the terminal under test)
2. iTerm2 screenshot (the reference/correct rendering)
3. Heatmap showing pixel-level differences (red=different, green=similar)

Analyze the visual differences and provide a structured assessment. Focus on:
- Text clarity and sharpness (blurring, smearing)
- Character alignment and spacing
- Box drawing character rendering
- Color accuracy
- Unicode/emoji rendering
- Overall visual fidelity

Respond in this exact JSON format:
{{
    "verdict": "PASS" | "MINOR_ISSUES" | "FAIL",
    "score": <0-100 integer>,
    "summary": "<one sentence summary>",
    "issues": [
        {{
            "category": "text_clarity" | "box_drawing" | "colors" | "unicode" | "spacing" | "other",
            "severity": "minor" | "moderate" | "major",
            "description": "<specific description of the issue>"
        }}
    ],
    "details": "<2-3 sentences with specific observations>"
}}

Be precise and technical. If the rendering is identical or nearly identical, say so. If there are issues, describe them specifically."""

    def _parse_response(self, text: str) -> Dict:
        """Parse LLM response to extract JSON."""
        import re
        json_match = re.search(r'\{[\s\S]*\}', text)
        if json_match:
            try:
                result = json.loads(json_match.group())
                result['judge'] = self.name
                result['raw_response'] = text
                return result
            except json.JSONDecodeError:
                pass

        return {
            "judge": self.name,
            "verdict": "PARSE_ERROR",
            "score": 0,
            "summary": "Failed to parse LLM response",
            "raw_response": text
        }


# =============================================================================
# Test Runner
# =============================================================================

class VisualTestRunner:
    """Orchestrates the visual comparison tests."""

    def __init__(self, use_opus: bool = True, use_gpt: bool = True):
        self.judges: List[LLMJudge] = []
        self.test_dir: Optional[Path] = None

        if use_opus:
            try:
                self.judges.append(AnthropicJudge())
                print(f"  [OK] Initialized Claude Opus 4.5 judge")
            except Exception as e:
                print(f"  [WARN] Could not initialize Opus judge: {e}")

        if use_gpt:
            try:
                self.judges.append(OpenAIJudge())
                print(f"  [OK] Initialized GPT-4o judge")
            except Exception as e:
                print(f"  [WARN] Could not initialize GPT judge: {e}")

        if not self.judges:
            raise RuntimeError("No LLM judges available. Set ANTHROPIC_API_KEY or OPENAI_API_KEY.")

    def setup(self) -> Tuple[int, int]:
        """Set up test environment and return window IDs for both terminals."""
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        self.test_dir = OUTPUT_DIR / timestamp
        self.test_dir.mkdir(parents=True, exist_ok=True)

        print(f"\n{'='*70}")
        print("DashTerm2 Visual Comparison Test with LLM Judge")
        print(f"{'='*70}")
        print(f"Output: {self.test_dir}")
        print(f"Judges: {', '.join(j.name for j in self.judges)}")

        # Clean up existing instances
        print("\n[1/4] Cleaning up existing instances...")
        subprocess.run(['pkill', '-9', 'DashTerm2'], capture_output=True)
        subprocess.run(['pkill', '-9', 'iTermServer'], capture_output=True)
        subprocess.run(['rm', '-f'] + list(Path.home().glob('Library/Application Support/DashTerm2/*.socket')),
                      capture_output=True)
        time.sleep(1)

        # Find DashTerm2 build
        import glob
        dashterm_matches = glob.glob(os.path.expanduser(DASHTERM_APP_PATTERN))
        if not dashterm_matches:
            raise RuntimeError("DashTerm2 build not found. Build first with xcodebuild.")
        dashterm_app = dashterm_matches[0]

        # Launch applications
        print("[2/4] Launching terminal applications...")
        subprocess.run(['open', dashterm_app])
        subprocess.run(['open', '-a', 'iTerm'])
        time.sleep(4)

        # Get window IDs
        print("[3/4] Locating terminal windows...")
        dashterm_wid = get_window_id("DashTerm")
        iterm_wid = get_window_id("iTerm")

        if not dashterm_wid:
            raise RuntimeError("Could not find DashTerm2 window")
        if not iterm_wid:
            raise RuntimeError("Could not find iTerm2 window")

        print(f"  DashTerm2 window ID: {dashterm_wid}")
        print(f"  iTerm2 window ID: {iterm_wid}")

        # Clear both terminals
        send_command("DashTerm2", "clear")
        send_command("iTerm", "clear")
        time.sleep(1)

        return dashterm_wid, iterm_wid

    def run_single_test(
        self,
        test_name: str,
        test_description: str,
        command: str,
        wait_time: int,
        dashterm_wid: int,
        iterm_wid: int
    ) -> Dict:
        """Run a single visual comparison test."""

        print(f"\n  Testing: {test_name}")
        print(f"    Command: {command[:60]}{'...' if len(command) > 60 else ''}")

        # Send command to both terminals
        send_command("DashTerm2", command)
        send_command("iTerm", command)
        time.sleep(wait_time)

        # Capture screenshots
        dashterm_path = self.test_dir / f"{test_name}_dashterm2.png"
        iterm_path = self.test_dir / f"{test_name}_iterm2.png"

        capture_window(dashterm_wid, dashterm_path)
        capture_window(iterm_wid, iterm_path)

        if not dashterm_path.exists() or not iterm_path.exists():
            return {"test_name": test_name, "error": "Screenshot capture failed"}

        # Load and process images
        dashterm_img = Image.open(dashterm_path)
        iterm_img = Image.open(iterm_path)

        # Extract content areas (remove title bars)
        dashterm_content = extract_terminal_content(dashterm_img)
        iterm_content = extract_terminal_content(iterm_img)

        # Create heatmap diff
        heatmap_img, similarity, stats = create_heatmap_diff(dashterm_content, iterm_content)

        # Save heatmap and comparison panel
        heatmap_path = self.test_dir / f"{test_name}_heatmap.png"
        heatmap_img.save(heatmap_path)

        comparison_panel = create_comparison_panel(
            dashterm_content, iterm_content, heatmap_img,
            test_name, similarity
        )
        comparison_path = self.test_dir / f"{test_name}_comparison.png"
        comparison_panel.save(comparison_path)

        print(f"    Pixel similarity: {similarity:.1f}%")

        # Convert images to base64 for LLM evaluation
        dashterm_b64 = image_to_base64(dashterm_content)
        iterm_b64 = image_to_base64(iterm_content)
        heatmap_b64 = image_to_base64(heatmap_img)

        # Run LLM evaluations in parallel
        print(f"    Evaluating with {len(self.judges)} LLM judge(s)...")

        evaluations = []
        with ThreadPoolExecutor(max_workers=len(self.judges)) as executor:
            futures = {
                executor.submit(
                    judge.evaluate,
                    dashterm_b64, iterm_b64, heatmap_b64,
                    test_name, test_description, stats
                ): judge.name
                for judge in self.judges
            }

            for future in as_completed(futures):
                judge_name = futures[future]
                try:
                    result = future.result()
                    evaluations.append(result)
                    verdict = result.get('verdict', 'UNKNOWN')
                    score = result.get('score', 0)
                    color = '\033[92m' if verdict == 'PASS' else '\033[93m' if verdict == 'MINOR_ISSUES' else '\033[91m'
                    print(f"      {judge_name}: {color}{verdict}\033[0m (score: {score}/100)")
                except Exception as e:
                    print(f"      {judge_name}: \033[91mERROR\033[0m - {e}")
                    evaluations.append({"judge": judge_name, "error": str(e)})

        # Compile results
        result = {
            "test_name": test_name,
            "description": test_description,
            "command": command,
            "pixel_stats": stats,
            "pixel_similarity": similarity,
            "evaluations": evaluations,
            "files": {
                "dashterm": str(dashterm_path),
                "iterm": str(iterm_path),
                "heatmap": str(heatmap_path),
                "comparison": str(comparison_path)
            }
        }

        return result

    def run_all_tests(self, test_cases: List[Tuple] = None) -> Dict:
        """Run all visual comparison tests."""

        if test_cases is None:
            test_cases = TEST_CASES

        dashterm_wid, iterm_wid = self.setup()

        print(f"\n[4/4] Running {len(test_cases)} visual tests...")

        results = []
        for test_name, description, command, wait_time in test_cases:
            result = self.run_single_test(
                test_name, description, command, wait_time,
                dashterm_wid, iterm_wid
            )
            results.append(result)

            # Clear for next test
            send_command("DashTerm2", "clear")
            send_command("iTerm", "clear")
            time.sleep(0.5)

        # Generate summary report
        report = self._generate_report(results)

        # Save results
        results_path = self.test_dir / "results.json"
        with open(results_path, 'w') as f:
            json.dump({"results": results, "report": report}, f, indent=2)

        report_path = self.test_dir / "report.md"
        with open(report_path, 'w') as f:
            f.write(self._format_markdown_report(results, report))

        print(f"\n{'='*70}")
        print("SUMMARY")
        print(f"{'='*70}")
        print(f"  Tests run: {len(results)}")
        print(f"  Average pixel similarity: {report['avg_pixel_similarity']:.1f}%")
        print(f"  Average LLM score: {report['avg_llm_score']:.1f}/100")
        print(f"\n  Verdicts:")
        for verdict, count in report['verdict_counts'].items():
            color = '\033[92m' if verdict == 'PASS' else '\033[93m' if verdict == 'MINOR_ISSUES' else '\033[91m'
            print(f"    {color}{verdict}\033[0m: {count}")
        print(f"\n  Results: {results_path}")
        print(f"  Report:  {report_path}")
        print(f"  Images:  {self.test_dir}")

        return {"results": results, "report": report}

    def _generate_report(self, results: List[Dict]) -> Dict:
        """Generate summary statistics from test results."""

        pixel_similarities = [r.get('pixel_similarity', 0) for r in results if 'pixel_similarity' in r]

        all_scores = []
        all_verdicts = []
        for r in results:
            for eval in r.get('evaluations', []):
                if 'score' in eval:
                    all_scores.append(eval['score'])
                if 'verdict' in eval:
                    all_verdicts.append(eval['verdict'])

        verdict_counts = {}
        for v in all_verdicts:
            verdict_counts[v] = verdict_counts.get(v, 0) + 1

        return {
            "total_tests": len(results),
            "avg_pixel_similarity": sum(pixel_similarities) / len(pixel_similarities) if pixel_similarities else 0,
            "avg_llm_score": sum(all_scores) / len(all_scores) if all_scores else 0,
            "verdict_counts": verdict_counts,
            "timestamp": datetime.now().isoformat()
        }

    def _format_markdown_report(self, results: List[Dict], report: Dict) -> str:
        """Format results as a Markdown report."""

        lines = [
            "# DashTerm2 Visual Comparison Report",
            "",
            f"**Generated:** {report['timestamp']}",
            f"**Judges:** {', '.join(j.name for j in self.judges)}",
            "",
            "## Summary",
            "",
            f"- **Tests:** {report['total_tests']}",
            f"- **Average Pixel Similarity:** {report['avg_pixel_similarity']:.1f}%",
            f"- **Average LLM Score:** {report['avg_llm_score']:.1f}/100",
            "",
            "### Verdict Distribution",
            ""
        ]

        for verdict, count in report['verdict_counts'].items():
            emoji = "[check]" if verdict == "PASS" else "[warn]" if verdict == "MINOR_ISSUES" else "[x]"
            lines.append(f"- {emoji} **{verdict}:** {count}")

        lines.extend(["", "## Test Results", ""])

        for r in results:
            test_name = r.get('test_name', 'Unknown')
            desc = r.get('description', '')
            pixel_sim = r.get('pixel_similarity', 0)

            lines.extend([
                f"### {test_name}",
                "",
                f"**Description:** {desc}",
                f"**Pixel Similarity:** {pixel_sim:.1f}%",
                ""
            ])

            for eval in r.get('evaluations', []):
                judge = eval.get('judge', 'Unknown')
                verdict = eval.get('verdict', 'N/A')
                score = eval.get('score', 0)
                summary = eval.get('summary', '')
                details = eval.get('details', '')

                lines.extend([
                    f"**{judge}:** {verdict} ({score}/100)",
                    f"> {summary}",
                    ""
                ])

                if details:
                    lines.append(f"{details}")
                    lines.append("")

                issues = eval.get('issues', [])
                if issues:
                    lines.append("**Issues:**")
                    for issue in issues:
                        severity = issue.get('severity', 'unknown')
                        category = issue.get('category', 'other')
                        desc = issue.get('description', '')
                        lines.append(f"- [{severity}] {category}: {desc}")
                    lines.append("")

            lines.append("---")
            lines.append("")

        return "\n".join(lines)


# =============================================================================
# Main
# =============================================================================

def main():
    parser = argparse.ArgumentParser(
        description="DashTerm2 Visual Comparison Test with LLM-as-Judge",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
    python3 llm_visual_judge.py --all                  # Run all tests with both LLMs
    python3 llm_visual_judge.py --all --opus-only      # Use only Anthropic Claude
    python3 llm_visual_judge.py --test-case box_drawing_single  # Run single test

Environment Variables:
    ANTHROPIC_API_KEY - Required for Anthropic Claude
    OPENAI_API_KEY    - Required for OpenAI GPT
    ANTHROPIC_MODEL   - Override model (default: claude-opus-4-5-20250514)
    OPENAI_MODEL      - Override model (default: gpt-5.2)
"""
    )

    parser.add_argument('--all', action='store_true', help='Run all test cases')
    parser.add_argument('--test-case', type=str, help='Run a specific test case by name')
    parser.add_argument('--opus-only', action='store_true', help='Use only Anthropic Claude')
    parser.add_argument('--gpt-only', action='store_true', help='Use only OpenAI GPT')
    parser.add_argument('--list-tests', action='store_true', help='List available test cases')

    args = parser.parse_args()

    if args.list_tests:
        print("\nAvailable test cases:")
        for name, desc, cmd, _ in TEST_CASES:
            print(f"  {name}: {desc}")
        return

    if not args.all and not args.test_case:
        parser.print_help()
        return

    # Determine which judges to use
    use_opus = not args.gpt_only
    use_gpt = not args.opus_only

    print("\nInitializing LLM judges...")
    runner = VisualTestRunner(use_opus=use_opus, use_gpt=use_gpt)

    if args.test_case:
        # Find the specific test case
        test_cases = [(n, d, c, w) for n, d, c, w in TEST_CASES if n == args.test_case]
        if not test_cases:
            print(f"Error: Test case '{args.test_case}' not found")
            print("Use --list-tests to see available test cases")
            sys.exit(1)
        runner.run_all_tests(test_cases)
    else:
        runner.run_all_tests()


if __name__ == "__main__":
    main()
