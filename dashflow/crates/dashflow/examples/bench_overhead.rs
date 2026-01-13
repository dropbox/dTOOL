//! Microbenchmark to isolate overhead sources in graph execution
//!
//! Run with: cargo run --example bench_overhead --release

// Benchmarks must panic on setup failure - error handling would distort measurements
#![allow(clippy::expect_used)]

use std::hint::black_box;
use std::time::{Duration, Instant};
use tracing::info_span;
use uuid::Uuid;

const ITERATIONS: u64 = 100_000;

fn bench<F: FnMut()>(name: &str, mut f: F) {
    // Warmup
    for _ in 0..1000 {
        f();
    }

    let start = Instant::now();
    for _ in 0..ITERATIONS {
        f();
    }
    let elapsed = start.elapsed();
    let ns_per_iter = elapsed.as_nanos() as f64 / ITERATIONS as f64;
    println!("{:40} {:>10.1} ns/iter", name, ns_per_iter);
}

fn main() {
    println!("=== Overhead Microbenchmark ({} iterations) ===\n", ITERATIONS);
    println!("{:40} {:>13}", "Test", "Time");
    println!("{:-<55}", "");

    // Baseline: empty function
    bench("baseline (empty)", || {
        black_box(());
    });

    // UUID generation
    bench("Uuid::new_v4()", || {
        black_box(Uuid::new_v4());
    });

    // String allocation (simulating node_name.to_string())
    let node_name = "my_node_name";
    bench("node_name.to_string()", || {
        black_box(node_name.to_string());
    });

    // Tracing span creation WITHOUT subscriber
    bench("info_span! (no subscriber)", || {
        let span = info_span!(
            "test.span",
            field1 = "value1",
            field2 = 42,
            field3 = tracing::field::Empty
        );
        black_box(span);
    });

    // Tracing span creation with UUID (like graph.invoke)
    bench("info_span! + Uuid (graph pattern)", || {
        let request_id = Uuid::new_v4();
        let span = info_span!(
            "graph.invoke",
            request_id = %request_id,
            graph.name = "test_graph",
            graph.entry_point = "start",
            graph.duration_ms = tracing::field::Empty,
            graph.nodes_executed = tracing::field::Empty
        );
        black_box(span);
    });

    // Node span pattern (like node.execute)
    bench("info_span! (node pattern)", || {
        let span = info_span!(
            "node.execute",
            node.name = "test_node",
            input_size_bytes = 1024u64,
            output_size_bytes = tracing::field::Empty,
            retries_enabled = false
        );
        black_box(span);
    });

    // Clone of Option<RetryPolicy> pattern
    #[derive(Clone)]
    struct FakeRetryPolicy {
        _max_retries: u32,
        _base_delay: Duration,
    }
    let retry_policy = Some(FakeRetryPolicy {
        _max_retries: 3,
        _base_delay: Duration::from_millis(100),
    });
    bench("retry_policy.clone()", || {
        black_box(retry_policy.clone());
    });

    // Combined: what one node iteration costs
    bench("COMBINED: one node overhead", || {
        let _span = info_span!(
            "node.execute",
            node.name = "test_node",
            input_size_bytes = 1024u64,
            output_size_bytes = tracing::field::Empty,
            retries_enabled = false
        );
        let _name = "test_node".to_string();
        let _policy = retry_policy.clone();
        black_box(());
    });

    println!("\n{:-<55}", "");
    println!("Note: 1 ms = 1,000,000 ns");
    println!("If 5-node graph takes 133ms = 133,000,000 ns");
    println!("That's 26,600,000 ns per node - but overhead here is ~100s of ns");
    println!("\nThe bulk of overhead must be elsewhere (tokio runtime, async, etc.)");

    println!("\n=== Testing tokio overhead ===\n");

    // Need a runtime for async tests
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("failed to build tokio runtime");

    // Test basic async block overhead
    {
        let start = Instant::now();
        for _ in 0..ITERATIONS {
            rt.block_on(async {
                black_box(1 + 1);
            });
        }
        let elapsed = start.elapsed();
        let ns_per_iter = elapsed.as_nanos() as f64 / ITERATIONS as f64;
        println!("{:40} {:>10.1} ns/iter", "rt.block_on(async { simple })", ns_per_iter);
    }

    // Test spawn overhead
    rt.block_on(async {
        let start = Instant::now();
        for _ in 0..ITERATIONS {
            let handle = tokio::spawn(async {
                black_box(1 + 1);
            });
            handle.await.expect("spawned task panicked");
        }
        let elapsed = start.elapsed();
        let ns_per_iter = elapsed.as_nanos() as f64 / ITERATIONS as f64;
        println!("{:40} {:>10.1} ns/iter", "tokio::spawn + await", ns_per_iter);
    });

    // Test timeout wrapper
    rt.block_on(async {
        let start = Instant::now();
        for _ in 0..ITERATIONS {
            let result = tokio::time::timeout(
                Duration::from_secs(60),
                async { black_box(1 + 1) }
            ).await;
            let _ = black_box(result);
        }
        let elapsed = start.elapsed();
        let ns_per_iter = elapsed.as_nanos() as f64 / ITERATIONS as f64;
        println!("{:40} {:>10.1} ns/iter", "tokio::time::timeout(60s, async)", ns_per_iter);
    });

    // Test actual node execution pattern
    rt.block_on(async {
        let start = Instant::now();
        let timeout_duration = Duration::from_secs(60);
        for _ in 0..ITERATIONS {
            let _span = info_span!("node.execute", node.name = "test");
            let result = tokio::time::timeout(
                timeout_duration,
                async { black_box(1 + 1) }
            ).await;
            let _ = black_box(result);
        }
        let elapsed = start.elapsed();
        let ns_per_iter = elapsed.as_nanos() as f64 / ITERATIONS as f64;
        println!("{:40} {:>10.1} ns/iter", "COMBINED: span + timeout + async", ns_per_iter);
    });
}
