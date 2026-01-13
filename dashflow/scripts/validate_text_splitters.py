#!/usr/bin/env python3
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
"""
Validate Rust text splitter implementations against Python baseline.

Compares output of Rust CharacterTextSplitter, RecursiveCharacterTextSplitter,
MarkdownTextSplitter, and HTMLTextSplitter with Python equivalents.

Usage:
    python3 scripts/validate_text_splitters.py

Requirements:
    pip install dashflow-text-splitters
"""

import json
import sys
from typing import List, Dict, Any

try:
    from dashflow_text_splitters import (
        CharacterTextSplitter,
        RecursiveCharacterTextSplitter,
        MarkdownTextSplitter,
        HTMLTextSplitter,
    )
except ImportError:
    print("ERROR: dashflow-text-splitters not installed", file=sys.stderr)
    print("Install with: pip install dashflow-text-splitters", file=sys.stderr)
    sys.exit(1)


def test_character_splitter():
    """Test CharacterTextSplitter with various configurations."""
    print("\n=== CharacterTextSplitter Tests ===\n")

    text = "Line 1\n\nLine 2\n\nLine 3\n\nLine 4"

    # Test 1: Basic double newline separator
    splitter = CharacterTextSplitter(
        separator="\n\n",
        chunk_size=20,
        chunk_overlap=0,
        length_function=len,
    )
    chunks = splitter.split_text(text)
    print("Test 1 - Basic double newline:")
    print(f"Input: {repr(text)}")
    print(f"Chunk size: 20, Overlap: 0")
    print(f"Output chunks ({len(chunks)}):")
    for i, chunk in enumerate(chunks):
        print(f"  [{i}] ({len(chunk)} chars): {repr(chunk)}")

    # Test 2: With overlap
    splitter = CharacterTextSplitter(
        separator="\n\n",
        chunk_size=20,
        chunk_overlap=5,
        length_function=len,
    )
    chunks = splitter.split_text(text)
    print("\nTest 2 - With overlap:")
    print(f"Input: {repr(text)}")
    print(f"Chunk size: 20, Overlap: 5")
    print(f"Output chunks ({len(chunks)}):")
    for i, chunk in enumerate(chunks):
        print(f"  [{i}] ({len(chunk)} chars): {repr(chunk)}")


def test_recursive_splitter():
    """Test RecursiveCharacterTextSplitter."""
    print("\n=== RecursiveCharacterTextSplitter Tests ===\n")

    text = "This is paragraph one.\n\nThis is paragraph two with more content.\n\nParagraph three."

    # Test 1: Default separators
    splitter = RecursiveCharacterTextSplitter(
        chunk_size=30,
        chunk_overlap=5,
        length_function=len,
    )
    chunks = splitter.split_text(text)
    print("Test 1 - Default separators:")
    print(f"Input: {repr(text)}")
    print(f"Chunk size: 30, Overlap: 5")
    print(f"Separators: {splitter._separators}")
    print(f"Output chunks ({len(chunks)}):")
    for i, chunk in enumerate(chunks):
        print(f"  [{i}] ({len(chunk)} chars): {repr(chunk)}")

    # Test 2: Small chunk size forces word-level splitting
    splitter = RecursiveCharacterTextSplitter(
        chunk_size=15,
        chunk_overlap=3,
        length_function=len,
    )
    chunks = splitter.split_text(text)
    print("\nTest 2 - Small chunks (word-level):")
    print(f"Input: {repr(text)}")
    print(f"Chunk size: 15, Overlap: 3")
    print(f"Output chunks ({len(chunks)}):")
    for i, chunk in enumerate(chunks):
        print(f"  [{i}] ({len(chunk)} chars): {repr(chunk)}")


def test_markdown_splitter():
    """Test MarkdownTextSplitter."""
    print("\n=== MarkdownTextSplitter Tests ===\n")

    text = """# Header 1

Some content under header 1.

## Header 2

Content under header 2.

```python
def hello():
    print("Hello")
```

More text after code block.

### Header 3

Final content."""

    # Test 1: Basic markdown splitting
    splitter = MarkdownTextSplitter(
        chunk_size=100,
        chunk_overlap=20,
    )
    chunks = splitter.split_text(text)
    print("Test 1 - Basic markdown:")
    print(f"Input length: {len(text)} chars")
    print(f"Chunk size: 100, Overlap: 20")
    print(f"Separators: {splitter._separators}")
    print(f"Output chunks ({len(chunks)}):")
    for i, chunk in enumerate(chunks):
        print(f"  [{i}] ({len(chunk)} chars):")
        # Show first 80 chars of each chunk
        preview = chunk[:80].replace('\n', '\\n')
        if len(chunk) > 80:
            preview += "..."
        print(f"    {repr(preview)}")

    # Test 2: Smaller chunks
    splitter = MarkdownTextSplitter(
        chunk_size=50,
        chunk_overlap=10,
    )
    chunks = splitter.split_text(text)
    print("\nTest 2 - Smaller chunks:")
    print(f"Chunk size: 50, Overlap: 10")
    print(f"Output chunks ({len(chunks)}):")
    for i, chunk in enumerate(chunks):
        preview = chunk[:50].replace('\n', '\\n')
        if len(chunk) > 50:
            preview += "..."
        print(f"  [{i}] ({len(chunk)} chars): {repr(preview)}")


def test_html_splitter():
    """Test HTMLTextSplitter."""
    print("\n=== HTMLTextSplitter Tests ===\n")

    text = """<html>
<head><title>Test Page</title></head>
<body>
<h1>Main Header</h1>
<p>First paragraph with some content.</p>
<div>
<h2>Subheader</h2>
<p>Second paragraph inside a div.</p>
<ul>
<li>List item 1</li>
<li>List item 2</li>
</ul>
</div>
</body>
</html>"""

    # Test 1: Basic HTML splitting
    splitter = HTMLTextSplitter(
        chunk_size=100,
        chunk_overlap=20,
    )
    chunks = splitter.split_text(text)
    print("Test 1 - Basic HTML:")
    print(f"Input length: {len(text)} chars")
    print(f"Chunk size: 100, Overlap: 20")
    print(f"Separators: {splitter._separators}")
    print(f"Output chunks ({len(chunks)}):")
    for i, chunk in enumerate(chunks):
        print(f"  [{i}] ({len(chunk)} chars):")
        preview = chunk[:80].replace('\n', '\\n')
        if len(chunk) > 80:
            preview += "..."
        print(f"    {repr(preview)}")

    # Test 2: Smaller chunks
    splitter = HTMLTextSplitter(
        chunk_size=60,
        chunk_overlap=10,
    )
    chunks = splitter.split_text(text)
    print("\nTest 2 - Smaller chunks:")
    print(f"Chunk size: 60, Overlap: 10")
    print(f"Output chunks ({len(chunks)}):")
    for i, chunk in enumerate(chunks):
        preview = chunk[:60].replace('\n', '\\n')
        if len(chunk) > 60:
            preview += "..."
        print(f"  [{i}] ({len(chunk)} chars): {repr(preview)}")


def export_test_cases():
    """Export test cases to JSON for Rust tests."""
    print("\n=== Exporting Test Cases to JSON ===\n")

    test_cases = {
        "character_splitter": [],
        "recursive_splitter": [],
        "markdown_splitter": [],
        "html_splitter": []
    }

    # Character splitter test cases
    text = "Line 1\n\nLine 2\n\nLine 3\n\nLine 4"
    splitter = CharacterTextSplitter(separator="\n\n", chunk_size=20, chunk_overlap=0)
    test_cases["character_splitter"].append({
        "name": "basic_double_newline",
        "text": text,
        "separator": "\n\n",
        "chunk_size": 20,
        "chunk_overlap": 0,
        "expected": splitter.split_text(text)
    })

    splitter = CharacterTextSplitter(separator="\n\n", chunk_size=20, chunk_overlap=5)
    test_cases["character_splitter"].append({
        "name": "with_overlap",
        "text": text,
        "separator": "\n\n",
        "chunk_size": 20,
        "chunk_overlap": 5,
        "expected": splitter.split_text(text)
    })

    # Recursive splitter test cases
    text = "This is paragraph one.\n\nThis is paragraph two with more content.\n\nParagraph three."
    splitter = RecursiveCharacterTextSplitter(chunk_size=30, chunk_overlap=5)
    test_cases["recursive_splitter"].append({
        "name": "default_separators",
        "text": text,
        "chunk_size": 30,
        "chunk_overlap": 5,
        "expected": splitter.split_text(text)
    })

    # Markdown splitter test cases
    text = """# Header 1

Some content under header 1.

## Header 2

Content under header 2."""
    splitter = MarkdownTextSplitter(chunk_size=100, chunk_overlap=20)
    test_cases["markdown_splitter"].append({
        "name": "basic_headers",
        "text": text,
        "chunk_size": 100,
        "chunk_overlap": 20,
        "expected": splitter.split_text(text)
    })

    # HTML splitter test cases
    text = """<html><body><h1>Header</h1><p>First paragraph.</p><p>Second paragraph.</p></body></html>"""
    splitter = HTMLTextSplitter(chunk_size=100, chunk_overlap=20)
    test_cases["html_splitter"].append({
        "name": "basic_html",
        "text": text,
        "chunk_size": 100,
        "chunk_overlap": 20,
        "expected": splitter.split_text(text)
    })

    output_file = "scripts/text_splitter_test_cases.json"
    with open(output_file, 'w') as f:
        json.dump(test_cases, f, indent=2)

    print(f"Exported test cases to: {output_file}")
    print(f"  - {len(test_cases['character_splitter'])} character splitter tests")
    print(f"  - {len(test_cases['recursive_splitter'])} recursive splitter tests")
    print(f"  - {len(test_cases['markdown_splitter'])} markdown splitter tests")
    print(f"  - {len(test_cases['html_splitter'])} html splitter tests")


def main():
    """Run all validation tests."""
    print("=" * 70)
    print("Python DashFlow Text Splitter Validation")
    print("=" * 70)

    try:
        test_character_splitter()
        test_recursive_splitter()
        test_markdown_splitter()
        test_html_splitter()
        export_test_cases()

        print("\n" + "=" * 70)
        print("Validation complete!")
        print("=" * 70)
        print("\nNext steps:")
        print("1. Compare this output with Rust implementation")
        print("2. Use text_splitter_test_cases.json for automated testing")
        print("3. Document any behavioral differences")

    except Exception as e:
        print(f"\nERROR: {e}", file=sys.stderr)
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()
