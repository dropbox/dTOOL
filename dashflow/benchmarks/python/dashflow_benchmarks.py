#!/usr/bin/env python3
"""
Python DashFlow Benchmarks

Mirrors the Rust benchmarks in crates/dashflow-dashflow/benches/graph_benchmarks.rs
to enable direct performance comparison.

Run with: python benchmarks/python/dashflow_benchmarks.py
"""

import time
import statistics
from typing import TypedDict, Annotated
from dataclasses import dataclass
from langgraph.graph import StateGraph, END  # External package
from langgraph.checkpoint.memory import MemorySaver  # External package
from operator import add
import json


# ============================================================================
# State Definitions (matching Rust benchmarks)
# ============================================================================

class SimpleState(TypedDict):
    """Simple state for basic benchmarks"""
    counter: int
    data: str


def merge_metadata(left: dict, right: dict) -> dict:
    """Merge two metadata dicts"""
    result = left.copy() if left else {}
    if right:
        result.update(right)
    return result


class ComplexState(TypedDict):
    """Complex state for realistic benchmarks"""
    messages: Annotated[list[str], add]  # Merge lists by concatenating
    metadata: Annotated[dict[str, str], merge_metadata]  # Merge dicts by combining
    counter: int  # Last value wins
    status: str  # Last value wins
    next: str  # Last value wins


class SmallState(TypedDict):
    """Small state for cloning benchmarks (< 1 KB)"""
    counter: int
    status: str
    value: float


class MediumState(TypedDict):
    """Medium state for cloning benchmarks (1-10 KB)"""
    data: list[str]
    metadata: dict[str, str]
    counters: list[int]


class LargeState(TypedDict):
    """Large state for cloning benchmarks (> 100 KB)"""
    messages: list[str]
    metadata: dict[str, str]
    data_blocks: list[bytes]


# ============================================================================
# Helper Functions
# ============================================================================

@dataclass
class BenchmarkResult:
    """Store benchmark results"""
    name: str
    mean_ms: float
    median_ms: float
    std_dev_ms: float
    min_ms: float
    max_ms: float
    iterations: int


def benchmark(name: str, func, warmup: int = 5, iterations: int = 50) -> BenchmarkResult:
    """
    Run a benchmark function multiple times and collect statistics.

    Args:
        name: Benchmark name
        func: Function to benchmark (should be callable with no args)
        warmup: Number of warmup iterations
        iterations: Number of timed iterations

    Returns:
        BenchmarkResult with timing statistics
    """
    # Warmup
    for _ in range(warmup):
        func()

    # Timed runs
    times = []
    for _ in range(iterations):
        start = time.perf_counter()
        func()
        end = time.perf_counter()
        times.append((end - start) * 1000)  # Convert to milliseconds

    return BenchmarkResult(
        name=name,
        mean_ms=statistics.mean(times),
        median_ms=statistics.median(times),
        std_dev_ms=statistics.stdev(times) if len(times) > 1 else 0.0,
        min_ms=min(times),
        max_ms=max(times),
        iterations=iterations,
    )


# ============================================================================
# Graph Compilation Benchmarks
# ============================================================================

def bench_simple_graph_3_nodes():
    """Simple graph compilation (3 nodes, linear)"""
    graph = StateGraph(SimpleState)

    def node1(state: SimpleState) -> SimpleState:
        return {"counter": state["counter"] + 1, "data": state["data"]}

    def node2(state: SimpleState) -> SimpleState:
        return {"counter": state["counter"] + 1, "data": state["data"]}

    def node3(state: SimpleState) -> SimpleState:
        return {"counter": state["counter"] + 1, "data": state["data"]}

    graph.add_node("node1", node1)
    graph.add_node("node2", node2)
    graph.add_node("node3", node3)

    graph.add_edge("node1", "node2")
    graph.add_edge("node2", "node3")
    graph.add_edge("node3", END)
    graph.set_entry_point("node1")

    app = graph.compile()
    return app


def bench_complex_graph_10_nodes():
    """Complex graph compilation (10 nodes, mixed edges)"""
    graph = StateGraph(ComplexState)

    for i in range(10):
        def make_node(node_id):
            def node(state: ComplexState) -> ComplexState:
                return {
                    "counter": state["counter"] + 1,
                    "messages": state["messages"] + [f"Processed by node{node_id}"],
                    "metadata": state["metadata"],
                    "status": state["status"],
                    "next": state["next"],
                }
            return node

        graph.add_node(f"node{i}", make_node(i))

    for i in range(9):
        graph.add_edge(f"node{i}", f"node{i + 1}")
    graph.add_edge("node9", END)
    graph.set_entry_point("node0")

    app = graph.compile()
    return app


# ============================================================================
# Sequential Execution Benchmarks
# ============================================================================

def bench_3_nodes_simple_exec():
    """Execute 3-node simple graph"""
    app = bench_simple_graph_3_nodes()
    state = {"counter": 0, "data": ""}
    result = app.invoke(state)
    return result


def bench_5_nodes_complex_exec():
    """Execute 5-node complex graph"""
    graph = StateGraph(ComplexState)

    for i in range(5):
        def make_node(node_id):
            def node(state: ComplexState) -> ComplexState:
                metadata = state["metadata"].copy()
                metadata[f"step{node_id}"] = "completed"
                return {
                    "counter": state["counter"] + 1,
                    "messages": state["messages"] + [f"Processed by node{node_id}"],
                    "metadata": metadata,
                    "status": state["status"],
                    "next": state["next"],
                }
            return node

        graph.add_node(f"node{i}", make_node(i))

    for i in range(4):
        graph.add_edge(f"node{i}", f"node{i + 1}")
    graph.add_edge("node4", END)
    graph.set_entry_point("node0")

    app = graph.compile()
    state = {
        "messages": ["Initial message"],
        "metadata": {"user": "test_user", "session": "test_session"},
        "counter": 0,
        "status": "initialized",
        "next": "",
    }
    result = app.invoke(state)
    return result


# ============================================================================
# Conditional Branching Benchmarks
# ============================================================================

def bench_binary_conditional_exec():
    """Execute graph with binary conditional"""
    graph = StateGraph(ComplexState)

    def start(state: ComplexState) -> ComplexState:
        counter = state["counter"] + 1
        next_val = "even" if counter % 2 == 0 else "odd"
        return {
            "counter": counter,
            "messages": state["messages"],
            "metadata": state["metadata"],
            "status": state["status"],
            "next": next_val,
        }

    def even(state: ComplexState) -> ComplexState:
        return {
            "counter": state["counter"],
            "messages": state["messages"] + ["Even path"],
            "metadata": state["metadata"],
            "status": state["status"],
            "next": state["next"],
        }

    def odd(state: ComplexState) -> ComplexState:
        return {
            "counter": state["counter"],
            "messages": state["messages"] + ["Odd path"],
            "metadata": state["metadata"],
            "status": state["status"],
            "next": state["next"],
        }

    graph.add_node("start", start)
    graph.add_node("even", even)
    graph.add_node("odd", odd)

    def route_fn(state: ComplexState) -> str:
        return state["next"]

    graph.add_conditional_edges("start", route_fn, {"even": "even", "odd": "odd"})
    graph.add_edge("even", END)
    graph.add_edge("odd", END)
    graph.set_entry_point("start")

    app = graph.compile()
    state = {
        "messages": [],
        "metadata": {},
        "counter": 0,
        "status": "",
        "next": "",
    }
    result = app.invoke(state)
    return result


def bench_loop_with_exit_condition_exec():
    """Execute graph with loop and exit condition (5 iterations)"""
    graph = StateGraph(ComplexState)

    def processor(state: ComplexState) -> ComplexState:
        counter = state["counter"] + 1
        next_val = "end" if counter >= 5 else "continue"
        return {
            "counter": counter,
            "messages": state["messages"] + [f"Iteration {counter}"],
            "metadata": state["metadata"],
            "status": state["status"],
            "next": next_val,
        }

    graph.add_node("processor", processor)

    def route_fn(state: ComplexState) -> str:
        return state["next"]

    graph.add_conditional_edges(
        "processor",
        route_fn,
        {"continue": "processor", "end": END}
    )
    graph.set_entry_point("processor")

    app = graph.compile()
    state = {
        "messages": [],
        "metadata": {},
        "counter": 0,
        "status": "",
        "next": "",
    }
    result = app.invoke(state)
    return result


# ============================================================================
# Parallel Execution Benchmarks
# ============================================================================

def bench_fanout_3_workers_exec():
    """Execute graph with 3 parallel workers"""
    graph = StateGraph(ComplexState)

    def start(state: ComplexState) -> ComplexState:
        return {
            "counter": 0,
            "messages": state["messages"],
            "metadata": state["metadata"],
            "status": "distributing",
            "next": state["next"],
        }

    def make_worker(worker_id):
        def worker(state: ComplexState) -> ComplexState:
            metadata = {f"worker{worker_id}_result": "completed"}
            return {
                "messages": [f"Worker {worker_id} processing"],
                "metadata": metadata,
            }
        return worker

    def collect(state: ComplexState) -> ComplexState:
        return {
            "status": "collected",
            "counter": len(state["metadata"]),
        }

    graph.add_node("start", start)
    for i in range(1, 4):
        graph.add_node(f"worker{i}", make_worker(i))
    graph.add_node("collect", collect)

    # Fan-out to 3 workers
    for i in range(1, 4):
        graph.add_edge("start", f"worker{i}")

    # Fan-in from all workers to collector
    for i in range(1, 4):
        graph.add_edge(f"worker{i}", "collect")
    graph.add_edge("collect", END)

    graph.set_entry_point("start")

    app = graph.compile()
    state = {
        "messages": ["Initial message"],
        "metadata": {"user": "test_user", "session": "test_session"},
        "counter": 0,
        "status": "initialized",
        "next": "",
    }
    result = app.invoke(state)
    return result


# ============================================================================
# Checkpointing Benchmarks
# ============================================================================

def bench_memory_checkpoint_3_nodes_exec():
    """Execute 3-node graph with memory checkpointer"""
    graph = StateGraph(ComplexState)

    for i in range(1, 4):
        def make_node(node_id):
            def node(state: ComplexState) -> ComplexState:
                return {
                    "counter": state["counter"] + 1,
                    "messages": state["messages"] + [f"Node {node_id} completed"],
                    "metadata": state["metadata"],
                    "status": state["status"],
                    "next": state["next"],
                }
            return node

        graph.add_node(f"node{i}", make_node(i))

    graph.add_edge("node1", "node2")
    graph.add_edge("node2", "node3")
    graph.add_edge("node3", END)
    graph.set_entry_point("node1")

    checkpointer = MemorySaver()
    app = graph.compile(checkpointer=checkpointer)

    state = {
        "messages": ["Initial message"],
        "metadata": {"user": "test_user", "session": "test_session"},
        "counter": 0,
        "status": "initialized",
        "next": "",
    }
    config = {"configurable": {"thread_id": "bench_thread_1"}}
    result = app.invoke(state, config)
    return result


def bench_memory_checkpoint_5_nodes_exec():
    """Execute 5-node graph with memory checkpointer"""
    graph = StateGraph(ComplexState)

    for i in range(1, 6):
        def make_node(node_id):
            def node(state: ComplexState) -> ComplexState:
                metadata = state["metadata"].copy()
                for j in range(10):
                    metadata[f"node{node_id}_item{j}"] = "value"
                return {
                    "counter": state["counter"] + 1,
                    "messages": state["messages"] + [f"Node {node_id} completed"],
                    "metadata": metadata,
                    "status": state["status"],
                    "next": state["next"],
                }
            return node

        graph.add_node(f"node{i}", make_node(i))

    for i in range(1, 5):
        graph.add_edge(f"node{i}", f"node{i + 1}")
    graph.add_edge("node5", END)
    graph.set_entry_point("node1")

    checkpointer = MemorySaver()
    app = graph.compile(checkpointer=checkpointer)

    state = {
        "messages": ["Initial message"],
        "metadata": {"user": "test_user", "session": "test_session"},
        "counter": 0,
        "status": "initialized",
        "next": "",
    }
    config = {"configurable": {"thread_id": "bench_thread_2"}}
    result = app.invoke(state, config)
    return result


# ============================================================================
# Main Benchmark Runner
# ============================================================================

def run_all_benchmarks(warmup: int = 3, iterations: int = 30):
    """
    Run all benchmarks and collect results.

    Args:
        warmup: Number of warmup iterations per benchmark
        iterations: Number of timed iterations per benchmark

    Returns:
        List of BenchmarkResult objects
    """
    results = []

    print("Running Python DashFlow benchmarks...")
    print(f"Warmup: {warmup} iterations, Timed: {iterations} iterations")
    print("=" * 80)

    # Compilation benchmarks
    print("\n[1/11] Graph Compilation Benchmarks")
    results.append(benchmark(
        "compilation/simple_graph_3_nodes",
        bench_simple_graph_3_nodes,
        warmup,
        iterations
    ))

    results.append(benchmark(
        "compilation/complex_graph_10_nodes",
        bench_complex_graph_10_nodes,
        warmup,
        iterations
    ))

    # Sequential execution benchmarks
    print("[2/11] Sequential Execution Benchmarks")
    results.append(benchmark(
        "sequential_execution/3_nodes_simple",
        bench_3_nodes_simple_exec,
        warmup,
        iterations
    ))

    results.append(benchmark(
        "sequential_execution/5_nodes_complex",
        bench_5_nodes_complex_exec,
        warmup,
        iterations
    ))

    # Conditional branching benchmarks
    print("[3/11] Conditional Branching Benchmarks")
    results.append(benchmark(
        "conditional_branching/binary_conditional",
        bench_binary_conditional_exec,
        warmup,
        iterations
    ))

    results.append(benchmark(
        "conditional_branching/loop_with_exit_condition",
        bench_loop_with_exit_condition_exec,
        warmup,
        iterations
    ))

    # Parallel execution benchmarks
    print("[4/11] Parallel Execution Benchmarks")
    results.append(benchmark(
        "parallel_execution/fanout_3_workers",
        bench_fanout_3_workers_exec,
        warmup,
        iterations
    ))

    # Checkpointing benchmarks
    print("[5/11] Checkpointing Benchmarks")
    results.append(benchmark(
        "checkpointing/memory_checkpoint_3_nodes",
        bench_memory_checkpoint_3_nodes_exec,
        warmup,
        iterations
    ))

    results.append(benchmark(
        "checkpointing/memory_checkpoint_5_nodes",
        bench_memory_checkpoint_5_nodes_exec,
        warmup,
        iterations
    ))

    print("\n" + "=" * 80)
    print("All benchmarks complete!")

    return results


def print_results(results: list[BenchmarkResult]):
    """Print benchmark results in a formatted table"""
    print("\n" + "=" * 80)
    print("BENCHMARK RESULTS")
    print("=" * 80)
    print(f"{'Benchmark':<50} {'Mean (ms)':<12} {'Median (ms)':<12} {'Std Dev':<10}")
    print("-" * 80)

    for result in results:
        print(f"{result.name:<50} {result.mean_ms:>10.3f}  {result.median_ms:>10.3f}  {result.std_dev_ms:>8.3f}")

    print("=" * 80)


def save_results_json(results: list[BenchmarkResult], filename: str = "python_bench_results.json"):
    """Save results to JSON file"""
    data = {
        "benchmarks": [
            {
                "name": r.name,
                "mean_ms": r.mean_ms,
                "median_ms": r.median_ms,
                "std_dev_ms": r.std_dev_ms,
                "min_ms": r.min_ms,
                "max_ms": r.max_ms,
                "iterations": r.iterations,
            }
            for r in results
        ]
    }

    with open(filename, "w") as f:
        json.dump(data, f, indent=2)

    print(f"\nResults saved to {filename}")


if __name__ == "__main__":
    import sys

    # Parse command-line args
    warmup = 3
    iterations = 30

    if len(sys.argv) > 1:
        iterations = int(sys.argv[1])
    if len(sys.argv) > 2:
        warmup = int(sys.argv[2])

    # Run benchmarks
    results = run_all_benchmarks(warmup=warmup, iterations=iterations)

    # Print results
    print_results(results)

    # Save to JSON
    save_results_json(results, "benchmarks/python/python_bench_results.json")
