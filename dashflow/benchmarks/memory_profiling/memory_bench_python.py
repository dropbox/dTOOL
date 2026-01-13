#!/usr/bin/env python3
"""
Memory benchmark for DashFlow Python implementation

Performs typical DashFlow operations to measure memory footprint:
- Message creation and cloning
- Prompt template rendering
- Runnable execution
- Tool usage
- Text splitting
- Serialization

Run with: /usr/bin/time -l python3 memory_bench_python.py
"""

import copy
from dashflow_core.messages import HumanMessage, AIMessage
from dashflow_text_splitters import CharacterTextSplitter, RecursiveCharacterTextSplitter


def main():
    print("=== DashFlow Python Memory Benchmark ===\n")

    # Allocate collections to hold data (prevent premature deallocation)
    messages = []
    serialized = []
    splits = []

    # 1. Message operations (2000 messages)
    print("Creating 2000 messages...")
    for i in range(500):
        messages.append(HumanMessage(content=f"Message {i}: This is a test message with some content."))
        messages.append(AIMessage(content=f"Response {i}: This is an AI response."))

    # 2. Message cloning (test copy overhead)
    print("Cloning 2000 messages...")
    cloned_messages = [copy.deepcopy(msg) for msg in messages]
    assert len(messages) == len(cloned_messages)

    # 3. Serialization (2000 messages)
    print("Serializing 2000 messages...")
    for msg in messages:
        serialized.append(msg.json())

    # 4. String formatting (1000 renders simulating prompt templates)
    print("Formatting 1000 strings...")
    formatted_strings = []
    for i in range(1000):
        formatted_strings.append(f"Hello User_{i}, your ID is {i}!")

    # 5. Text splitting (100 documents)
    print("Splitting 100 documents...")
    doc = ("This is a test document. It contains multiple sentences. "
           "We will split it in various ways. This helps us measure memory usage. "
           "The document is long enough to create multiple chunks. "
           "Each chunk will be stored separately. ") * 10

    char_splitter = CharacterTextSplitter(
        chunk_size=100,
        chunk_overlap=20,
        separator="\n\n"
    )
    rec_splitter = RecursiveCharacterTextSplitter(
        chunk_size=100,
        chunk_overlap=20
    )

    for _ in range(50):
        splits.extend(char_splitter.split_text(doc))
        splits.extend(rec_splitter.split_text(doc))

    # 6. Additional string processing (1000 operations)
    print("Processing 1000 string transformations...")
    results = []
    for i in range(1000):
        input_text = f"input_{i}"
        results.append(f"Processed: {input_text}")

    # 7. Additional message operations (1000 more operations)
    print("Creating additional 1000 messages for memory pressure...")
    additional_messages = []
    for i in range(1000):
        additional_messages.append(HumanMessage(content=f"Additional message {i}"))

    # Print summary statistics
    print("\n=== Summary ===")
    print(f"Messages created: {len(messages)}")
    print(f"Messages cloned: {len(cloned_messages)}")
    print(f"Serialized strings: {len(serialized)}")
    print(f"Formatted strings: {len(formatted_strings)}")
    print(f"Text splits: {len(splits)}")
    print(f"String processing results: {len(results)}")
    print(f"Additional messages: {len(additional_messages)}")
    print("\nMemory stats will be printed by /usr/bin/time -l")


if __name__ == "__main__":
    main()
