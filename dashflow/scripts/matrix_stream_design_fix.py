#!/usr/bin/env python3
"""
Quick GPT-4 Vision analysis of MatrixStream design inconsistency
Focuses on the green text that doesn't match the Dropbox Blue theme

Usage:
    export OPENAI_API_KEY="sk-..."
    python3 scripts/matrix_stream_design_fix.py
"""

import base64
import json
import os
import sys
from pathlib import Path

try:
    from openai import OpenAI
except ImportError:
    print("ERROR: Missing openai package. Install with:")
    print("  pip install openai")
    sys.exit(1)

if not os.environ.get("OPENAI_API_KEY"):
    print("ERROR: OPENAI_API_KEY environment variable not set")
    print("Set with: export OPENAI_API_KEY=\"sk-...\"")
    sys.exit(1)

def encode_image(image_path):
    """Encode image to base64"""
    with open(image_path, "rb") as f:
        return base64.b64encode(f.read()).decode('utf-8')

def main():
    client = OpenAI(api_key=os.environ.get("OPENAI_API_KEY"))

    # Path to matrix stream screenshot
    screenshot_path = Path("screenshots/design_critique/matrix_stream.png")

    if not screenshot_path.exists():
        print(f"❌ Screenshot not found: {screenshot_path}")
        return

    # Encode image
    base64_image = encode_image(screenshot_path)

    # Create prompt focusing on design inconsistency
    prompt = """You are a strict UI/UX designer. Analyze this "Raw JSON Stream" component.

**CRITICAL ISSUE:** The bright green text (#00ff41) does NOT match the professional Dropbox Blue (#0061FF) theme used throughout the rest of the dashboard.

**Your Task:**
1. Identify ALL instances of green text that should be changed to Dropbox Blue (#0061FF)
2. Recommend specific color changes for:
   - Header text
   - Timestamp
   - Hexdump output
   - "RAW:" label
   - Footer status text
   - Any other green elements

3. Suggest how to maintain readability while using Dropbox Blue
4. Rate the current color inconsistency severity (0.0-1.0, where 1.0 is "completely breaks the design")

Provide specific CSS color recommendations."""

    # Call GPT-4 Vision
    print("🤖 Analyzing MatrixStream design with GPT-4 Vision...")

    response = client.chat.completions.create(
        model="gpt-4o",
        messages=[{
            "role": "user",
            "content": [
                {"type": "text", "text": prompt},
                {
                    "type": "image_url",
                    "image_url": {
                        "url": f"data:image/png;base64,{base64_image}"
                    }
                }
            ]
        }],
        max_tokens=1000
    )

    feedback = response.choices[0].message.content

    print("\n" + "="*60)
    print("GPT-4 VISION FEEDBACK - MatrixStream Color Fix")
    print("="*60 + "\n")
    print(feedback)
    print("\n" + "="*60)

    # Save feedback
    output_path = Path("screenshots/design_critique/matrix_stream_fix_feedback.txt")
    output_path.write_text(feedback)
    print(f"\n✅ Feedback saved to: {output_path}")

if __name__ == "__main__":
    main()
