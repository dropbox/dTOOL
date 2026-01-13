#!/usr/bin/env python3
"""
Python DashFlow benchmarks for comparison with Rust implementation.

This script benchmarks equivalent operations to the Rust benchmarks in
crates/dashflow-benchmarks/benches/core_benchmarks.rs

Run with: python benchmarks/python_comparison/bench_core_operations.py
"""

import json
import sys
import time
from typing import Callable, List
import asyncio

# Add the baseline DashFlow to path
sys.path.insert(0, '/Users/ayates/dashflow/libs/core')
sys.path.insert(0, '/Users/ayates/dashflow/libs/text-splitters')

from dashflow_core.messages import HumanMessage, AIMessage
from dashflow_core.prompts import PromptTemplate
from dashflow_core.runnables import RunnableLambda, RunnablePassthrough, RunnableConfig
from dashflow_core.tools import tool
from dashflow_text_splitters import (
    CharacterTextSplitter,
    RecursiveCharacterTextSplitter,
)


class BenchmarkRunner:
    """Simple benchmark runner using timeit-like approach."""

    def __init__(self, warmup=10, iterations=1000):
        self.warmup = warmup
        self.iterations = iterations
        self.results = {}

    def bench(self, name: str, func: Callable):
        """Benchmark a synchronous function."""
        # Warmup
        for _ in range(self.warmup):
            func()

        # Measure
        start = time.perf_counter()
        for _ in range(self.iterations):
            func()
        end = time.perf_counter()

        total_time = end - start
        avg_time = total_time / self.iterations

        self.results[name] = {
            'total_s': total_time,
            'avg_s': avg_time,
            'avg_ns': avg_time * 1e9,
            'avg_us': avg_time * 1e6,
            'iterations': self.iterations,
        }

        return avg_time

    def bench_async(self, name: str, func: Callable):
        """Benchmark an async function."""
        async def run_warmup():
            for _ in range(self.warmup):
                await func()

        async def run_bench():
            start = time.perf_counter()
            for _ in range(self.iterations):
                await func()
            end = time.perf_counter()
            return end - start

        # Run warmup
        asyncio.run(run_warmup())

        # Measure
        total_time = asyncio.run(run_bench())
        avg_time = total_time / self.iterations

        self.results[name] = {
            'total_s': total_time,
            'avg_s': avg_time,
            'avg_ns': avg_time * 1e9,
            'avg_us': avg_time * 1e6,
            'iterations': self.iterations,
        }

        return avg_time

    def print_results(self):
        """Print benchmark results in a readable format."""
        print("\n" + "="*80)
        print("Python DashFlow Benchmark Results")
        print("="*80)

        for name, result in self.results.items():
            avg_ns = result['avg_ns']
            avg_us = result['avg_us']

            if avg_ns < 1000:
                print(f"{name:50s} {avg_ns:10.2f} ns")
            elif avg_us < 1000:
                print(f"{name:50s} {avg_us:10.2f} μs")
            else:
                print(f"{name:50s} {result['avg_s']*1000:10.2f} ms")

        print("="*80)
        print(f"Total benchmarks: {len(self.results)}")
        print("="*80)


def bench_message_serialization(runner: BenchmarkRunner):
    """Message serialization benchmarks."""
    print("\n[Message Serialization]")

    # Serialize human message
    runner.bench(
        "serialize_human_message_simple",
        lambda: HumanMessage(content="Hello, world!").json()
    )

    # Deserialize human message
    json_str = '{"type":"human","content":"Hello, world!"}'
    runner.bench(
        "deserialize_human_message_simple",
        lambda: HumanMessage.parse_raw(json_str)
    )

    # Serialize AI message
    runner.bench(
        "serialize_ai_message",
        lambda: AIMessage(content="I'll search for that.").json()
    )

    # Batch serialization
    def serialize_batch():
        msgs = [HumanMessage(content=f"Message {i}") for i in range(10)]
        return json.dumps([msg.dict() for msg in msgs])

    runner.bench("serialize_message_batch_10", serialize_batch)


def bench_config_operations(runner: BenchmarkRunner):
    """Config operations benchmarks."""
    print("\n[Config Operations]")

    # Create config with tags
    runner.bench(
        "create_config_with_tags",
        lambda: RunnableConfig(tags=["test", "benchmark"], run_name="bench_run")
    )

    # Create config with metadata
    runner.bench(
        "create_config_with_metadata",
        lambda: RunnableConfig(metadata={"key1": "value1", "key2": 42})
    )

    # Clone config
    config = RunnableConfig(tags=["test"], run_name="bench_run")
    runner.bench(
        "clone_config",
        lambda: config.copy()
    )


def bench_prompt_templates(runner: BenchmarkRunner):
    """Prompt template benchmarks."""
    print("\n[Prompt Templates]")

    # Simple template
    template = PromptTemplate.from_template("Hello, {name}!")
    runner.bench(
        "render_simple_fstring",
        lambda: template.format(name="World")
    )

    # Complex template
    template_complex = PromptTemplate.from_template(
        "User: {user}\nAge: {age}\nCity: {city}\nQuery: {query}"
    )
    runner.bench(
        "render_complex_template",
        lambda: template_complex.format(
            user="Alice", age="30", city="NYC", query="What's the weather?"
        )
    )

    # Long content template
    long_text = "Lorem ipsum dolor sit amet. " * 100
    template_long = PromptTemplate.from_template("Context: {context}\n\nQuestion: {question}")
    runner.bench(
        "render_template_long_content",
        lambda: template_long.format(context=long_text, question="Summarize")
    )


def bench_message_operations(runner: BenchmarkRunner):
    """Message operations benchmarks."""
    print("\n[Message Operations]")

    # Clone human message
    msg_human = HumanMessage(content="Hello, world!")
    runner.bench(
        "clone_human_message",
        lambda: msg_human.copy()
    )

    # Clone AI message
    msg_ai = AIMessage(content="Response from AI")
    runner.bench(
        "clone_ai_message",
        lambda: msg_ai.copy()
    )


def bench_runnable_operations(runner: BenchmarkRunner):
    """Runnable operations benchmarks."""
    print("\n[Runnable Operations]")

    # Simple lambda runnable
    lambda_runnable = RunnableLambda(lambda x: x.upper())
    runner.bench_async(
        "lambda_runnable_simple",
        lambda: lambda_runnable.ainvoke("hello world")
    )

    # Passthrough runnable
    passthrough = RunnablePassthrough()
    runner.bench_async(
        "passthrough_runnable",
        lambda: passthrough.ainvoke("test input")
    )

    # Batch processing
    lambda_batch = RunnableLambda(lambda x: x.upper())
    inputs = [f"input {i}" for i in range(10)]
    runner.bench_async(
        "runnable_batch_10",
        lambda: lambda_batch.abatch(inputs)
    )


def bench_tool_operations(runner: BenchmarkRunner):
    """Tool operations benchmarks."""
    print("\n[Tool Operations]")

    # Simple tool
    @tool
    def echo(text: str) -> str:
        """Returns input unchanged."""
        return text

    runner.bench_async(
        "tool_call_simple",
        lambda: echo.ainvoke({"text": "test input"})
    )

    # Tool with processing
    @tool
    def uppercase(text: str) -> str:
        """Converts to uppercase."""
        return text.upper()

    runner.bench_async(
        "tool_call_with_processing",
        lambda: uppercase.ainvoke({"text": "hello world"})
    )


def bench_text_splitters(runner: BenchmarkRunner):
    """Text splitter benchmarks."""
    print("\n[Text Splitters]")

    # Character text splitter - small text
    text_small = "This is a test. " * 10
    splitter_char = CharacterTextSplitter(chunk_size=50, chunk_overlap=10)
    runner.bench(
        "character_splitter_small",
        lambda: splitter_char.split_text(text_small)
    )

    # Character text splitter - medium text
    text_medium = "This is a test. " * 100
    runner.bench(
        "character_splitter_medium",
        lambda: splitter_char.split_text(text_medium)
    )

    # Character text splitter - large text
    text_large = "This is a test. " * 1000
    runner.bench(
        "character_splitter_large",
        lambda: splitter_char.split_text(text_large)
    )

    # Recursive character text splitter - small text
    splitter_recursive = RecursiveCharacterTextSplitter(
        chunk_size=100,
        chunk_overlap=20,
        separators=["\n\n", "\n", ". ", " ", ""]
    )
    runner.bench(
        "recursive_splitter_small",
        lambda: splitter_recursive.split_text(text_small)
    )

    # Recursive character text splitter - medium text
    runner.bench(
        "recursive_splitter_medium",
        lambda: splitter_recursive.split_text(text_medium)
    )

    # Recursive character text splitter - large text
    runner.bench(
        "recursive_splitter_large",
        lambda: splitter_recursive.split_text(text_large)
    )


def main():
    """Run all benchmarks."""
    print("Python DashFlow Core Benchmarks")
    print("=" * 80)
    print("Python version:", sys.version)
    print("Running benchmarks with 10 warmup iterations and 1000 measurement iterations")
    print("=" * 80)

    runner = BenchmarkRunner(warmup=10, iterations=1000)

    # Run all benchmark groups
    bench_message_serialization(runner)
    bench_config_operations(runner)
    bench_prompt_templates(runner)
    bench_message_operations(runner)
    bench_runnable_operations(runner)
    bench_tool_operations(runner)
    bench_text_splitters(runner)

    # Print summary
    runner.print_results()

    # Save results to JSON
    output_file = "benchmarks/python_comparison/results_python.json"
    with open(output_file, 'w') as f:
        json.dump(runner.results, f, indent=2)
    print(f"\nResults saved to: {output_file}")


if __name__ == "__main__":
    main()
