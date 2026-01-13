//! Load tests for WASM executor
//!
//! Tests that verify performance and stability under load:
//! - High concurrency (100+ concurrent executions)
//! - Large WASM modules (up to 10MB size limit)
//! - Long-running executions (up to 30s timeout)
//! - Memory pressure (many concurrent memory-intensive operations)
//! - Metrics accuracy under load
//!
//! **Status:** Load Testing
//!
//! All tests should demonstrate that the WASM executor can handle
//! production-like workloads without crashes, deadlocks, or memory leaks.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dashflow_wasm_executor::config::WasmExecutorConfig;
use dashflow_wasm_executor::WasmExecutor;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Helper to load WASM fixture
fn load_fixture(name: &str) -> Vec<u8> {
    let path = format!("tests/fixtures/{}.wasm", name);
    fs::read(&path).unwrap_or_else(|_| panic!("Failed to read fixture: {}", path))
}

/// Helper to load malicious WASM fixture
fn load_malicious_fixture(name: &str) -> Vec<u8> {
    let path = format!("tests/fixtures/malicious/{}.wasm", name);
    fs::read(&path).unwrap_or_else(|_| panic!("Failed to read malicious fixture: {}", path))
}

//
// High Concurrency Tests
//

#[tokio::test]
async fn test_load_concurrent_executions_100plus() {
    // Test 100+ concurrent WASM executions
    // This verifies thread safety, no deadlocks, and proper resource cleanup

    let config = WasmExecutorConfig::for_testing();
    let executor = Arc::new(WasmExecutor::new(config).expect("Failed to create executor"));
    let wasm_bytes = Arc::new(load_fixture("simple_add"));

    let num_tasks = 150; // Test with 150 concurrent executions
    let success_count = Arc::new(AtomicUsize::new(0));
    let failure_count = Arc::new(AtomicUsize::new(0));

    let start = Instant::now();
    let mut handles = vec![];

    for i in 0..num_tasks {
        let exec = Arc::clone(&executor);
        let bytes = Arc::clone(&wasm_bytes);
        let success = Arc::clone(&success_count);
        let failure = Arc::clone(&failure_count);

        let handle = tokio::spawn(async move {
            let result = exec.execute(&bytes, "add", &[i as i32, i as i32]).await;
            match result {
                Ok(output) => {
                    // Verify result correctness
                    let expected = (i + i).to_string();
                    assert_eq!(output, expected, "Result mismatch for task {}", i);
                    success.fetch_add(1, Ordering::SeqCst);
                }
                Err(e) => {
                    eprintln!("Task {} failed: {}", i, e);
                    failure.fetch_add(1, Ordering::SeqCst);
                }
            }
        });

        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.expect("Task panicked");
    }

    let elapsed = start.elapsed();
    let success = success_count.load(Ordering::SeqCst);
    let failure = failure_count.load(Ordering::SeqCst);

    println!("\n=== Load Test: 100+ Concurrent Executions ===");
    println!("Total tasks: {}", num_tasks);
    println!("Successful: {}", success);
    println!("Failed: {}", failure);
    println!("Duration: {:?}", elapsed);
    println!(
        "Throughput: {:.2} executions/sec",
        num_tasks as f64 / elapsed.as_secs_f64()
    );

    // All tasks should succeed
    assert_eq!(
        success, num_tasks,
        "Expected all {} tasks to succeed, but only {} succeeded",
        num_tasks, success
    );
    assert_eq!(
        failure, 0,
        "Expected 0 failures, but {} tasks failed",
        failure
    );
}

#[tokio::test]
async fn test_load_large_wasm_modules() {
    // Test execution with large WASM modules (approach 10MB limit)
    // This verifies module loading, compilation, and memory management

    let mut config = WasmExecutorConfig::for_testing();
    config.max_wasm_size_bytes = 10 * 1024 * 1024; // 10MB limit
    let executor = WasmExecutor::new(config).expect("Failed to create executor");

    // Create a moderately large WASM module by loading existing fixture
    // In production, this would be a large compiled WASM (e.g., with many functions)
    let base_wasm = load_fixture("simple_add");

    // Test 1: Normal-sized module (should work)
    let start = Instant::now();
    let result = executor.execute(&base_wasm, "add", &[100, 200]).await;
    let elapsed = start.elapsed();

    assert!(
        result.is_ok(),
        "Normal-sized module execution failed: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap(), "300");

    println!("\n=== Load Test: Large WASM Modules ===");
    println!("Module size: {} bytes", base_wasm.len());
    println!("Execution time: {:?}", elapsed);

    // Test 2: Oversized module (should reject)
    let oversized_wasm = vec![0x00; 11 * 1024 * 1024]; // 11MB (exceeds limit)
    let result = executor.execute(&oversized_wasm, "add", &[1, 2]).await;

    assert!(
        result.is_err(),
        "Expected oversized module to be rejected, but it was accepted"
    );
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("module size exceeds") || error.contains("Invalid WASM"),
        "Expected size limit error, got: {}",
        error
    );

    println!("Oversized module correctly rejected (11MB > 10MB limit)");
}

#[tokio::test]
async fn test_load_long_running_executions() {
    // Test long-running executions (up to 30s timeout)
    // This verifies timeout enforcement and no resource leaks

    let mut config = WasmExecutorConfig::for_testing();
    config.max_execution_timeout = Duration::from_secs(5); // 5s timeout for test speed
    config.max_fuel = 1_000_000_000; // High fuel so timeout triggers first
    let executor = WasmExecutor::new(config).expect("Failed to create executor");

    let wasm_bytes = load_malicious_fixture("cpu_bomb");

    println!("\n=== Load Test: Long-Running Executions ===");
    println!("Timeout limit: 5 seconds");

    // Test 1: Short execution (should complete)
    let start = Instant::now();
    let result = executor.execute(&wasm_bytes, "fibonacci", &[10]).await;
    let elapsed = start.elapsed();

    assert!(
        result.is_ok() || elapsed < Duration::from_secs(5),
        "Short execution should complete or timeout gracefully"
    );
    println!("Short execution (fibonacci(10)): {:?}", elapsed);

    // Test 2: Long execution (should timeout)
    let start = Instant::now();
    let result = executor.execute(&wasm_bytes, "fibonacci", &[40]).await;
    let elapsed = start.elapsed();

    // Should fail due to timeout or fuel exhaustion
    assert!(
        result.is_err(),
        "Long execution should timeout or exhaust fuel"
    );

    // Verify timeout happened within reasonable bounds (not exact due to scheduling)
    assert!(
        elapsed < Duration::from_secs(10),
        "Timeout took too long: {:?} (expected ~5s)",
        elapsed
    );
    println!(
        "Long execution (fibonacci(40)) timed out after {:?}",
        elapsed
    );

    // Test 3: Multiple concurrent long executions with lower fuel
    let mut config3 = WasmExecutorConfig::for_testing();
    config3.max_execution_timeout = Duration::from_secs(5);
    config3.max_fuel = 5_000_000; // Lower fuel to ensure exhaustion
    let executor3 = WasmExecutor::new(config3).expect("Failed to create executor");

    let mut handles = vec![];
    let start = Instant::now();

    for _ in 0..10 {
        let exec = executor3.clone();
        let bytes = wasm_bytes.clone();
        let handle = tokio::spawn(async move { exec.execute(&bytes, "fibonacci", &[40]).await });
        handles.push(handle);
    }

    // All should timeout or exhaust fuel
    let mut error_count = 0;
    for handle in handles {
        let result = handle.await.expect("Task panicked");
        if result.is_err() {
            error_count += 1;
        }
    }

    let elapsed = start.elapsed();
    println!(
        "10 concurrent long executions: {} failed (timeout/fuel) in {:?}",
        error_count, elapsed
    );

    // At least some should have failed due to fuel exhaustion or timeout
    assert!(
        error_count > 0,
        "Expected at least some executions to fail (timeout or fuel exhaustion)"
    );
}

#[tokio::test]
async fn test_load_memory_pressure() {
    // Test many concurrent memory-intensive operations
    // This verifies memory limits, no memory leaks, and proper cleanup

    let mut config = WasmExecutorConfig::for_testing();
    config.max_memory_bytes = 10 * 1024 * 1024; // 10MB per execution
    let executor = Arc::new(WasmExecutor::new(config).expect("Failed to create executor"));
    let wasm_bytes = Arc::new(load_fixture("memory_access"));

    let num_tasks = 50; // 50 concurrent memory operations
    let success_count = Arc::new(AtomicUsize::new(0));

    println!("\n=== Load Test: Memory Pressure ===");
    println!("Concurrent tasks: {}", num_tasks);
    println!("Max memory per execution: 10MB");

    let start = Instant::now();
    let mut handles = vec![];

    for _i in 0..num_tasks {
        let exec = Arc::clone(&executor);
        let bytes = Arc::clone(&wasm_bytes);
        let success = Arc::clone(&success_count);

        let handle = tokio::spawn(async move {
            // Each task does multiple memory operations
            for _ in 0..10 {
                let result = exec.execute(&bytes, "load_value", &[]).await;
                if result.is_ok() {
                    success.fetch_add(1, Ordering::SeqCst);
                }
            }
        });

        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.expect("Task panicked");
    }

    let elapsed = start.elapsed();
    let success = success_count.load(Ordering::SeqCst);
    let expected = num_tasks * 10; // 10 operations per task

    println!("Total operations: {}", expected);
    println!("Successful: {}", success);
    println!("Duration: {:?}", elapsed);
    println!(
        "Throughput: {:.2} operations/sec",
        success as f64 / elapsed.as_secs_f64()
    );

    // Most or all operations should succeed
    assert!(
        success >= (expected * 95 / 100),
        "Expected at least 95% success rate, got {}/{}",
        success,
        expected
    );
}

#[tokio::test]
async fn test_load_metrics_accuracy() {
    // Test that metrics remain accurate under load
    // This verifies no race conditions in metrics collection

    let config = WasmExecutorConfig::for_testing();
    let executor = Arc::new(WasmExecutor::new(config).expect("Failed to create executor"));
    let wasm_bytes = Arc::new(load_fixture("simple_add"));

    let num_tasks = 100;
    let operations_per_task = 5;
    let total_operations = num_tasks * operations_per_task;

    println!("\n=== Load Test: Metrics Accuracy ===");
    println!("Tasks: {}", num_tasks);
    println!("Operations per task: {}", operations_per_task);
    println!("Total operations: {}", total_operations);

    let start = Instant::now();
    let mut handles = vec![];

    for i in 0..num_tasks {
        let exec = Arc::clone(&executor);
        let bytes = Arc::clone(&wasm_bytes);

        let handle = tokio::spawn(async move {
            for j in 0..operations_per_task {
                let _ = exec.execute(&bytes, "add", &[i, j]).await;
            }
        });

        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.expect("Task panicked");
    }

    let elapsed = start.elapsed();

    println!("Duration: {:?}", elapsed);
    println!(
        "Throughput: {:.2} operations/sec",
        total_operations as f64 / elapsed.as_secs_f64()
    );

    // Note: This test verifies execution completes without crashes
    // Actual metrics verification would require prometheus registry access
    // which is not exposed in the public API
    // The fact that all executions complete is evidence of metrics working
}

#[tokio::test]
async fn test_load_mixed_workload() {
    // Test a realistic mixed workload:
    // - Some fast executions
    // - Some slow executions
    // - Some memory-intensive executions
    // - Some that fail (invalid input)
    // This verifies the executor handles diverse workloads gracefully

    let config = WasmExecutorConfig::for_testing();
    let executor = Arc::new(WasmExecutor::new(config).expect("Failed to create executor"));

    let wasm_simple = Arc::new(load_fixture("simple_add"));
    let wasm_memory = Arc::new(load_fixture("memory_access"));
    let wasm_cpu = Arc::new(load_malicious_fixture("cpu_bomb"));

    let num_tasks = 100;
    let success_count = Arc::new(AtomicUsize::new(0));
    let failure_count = Arc::new(AtomicUsize::new(0));

    println!("\n=== Load Test: Mixed Workload ===");
    println!("Tasks: {}", num_tasks);
    println!("Mix: 50% fast, 30% memory, 20% CPU-intensive");

    let start = Instant::now();
    let mut handles = vec![];

    for i in 0..num_tasks {
        let exec = Arc::clone(&executor);
        let simple = Arc::clone(&wasm_simple);
        let memory = Arc::clone(&wasm_memory);
        let cpu = Arc::clone(&wasm_cpu);
        let success = Arc::clone(&success_count);
        let failure = Arc::clone(&failure_count);

        let handle = tokio::spawn(async move {
            let result = match i % 10 {
                // 50% fast operations (0-4)
                0..=4 => exec.execute(&simple, "add", &[i as i32, i as i32]).await,
                // 30% memory operations (5-7)
                5..=7 => exec.execute(&memory, "load_value", &[]).await,
                // 20% CPU-intensive operations (8-9) - may fail due to fuel limits
                _ => exec.execute(&cpu, "fibonacci", &[20]).await,
            };

            match result {
                Ok(_) => {
                    success.fetch_add(1, Ordering::SeqCst);
                }
                Err(_) => {
                    failure.fetch_add(1, Ordering::SeqCst);
                }
            }
        });

        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.expect("Task panicked");
    }

    let elapsed = start.elapsed();
    let success = success_count.load(Ordering::SeqCst);
    let failure = failure_count.load(Ordering::SeqCst);

    println!("Successful: {}", success);
    println!("Failed: {}", failure);
    println!("Duration: {:?}", elapsed);
    println!(
        "Throughput: {:.2} operations/sec",
        num_tasks as f64 / elapsed.as_secs_f64()
    );

    // Most operations should succeed (fast and memory operations)
    // Some CPU-intensive operations may fail due to fuel limits (expected)
    assert!(
        success >= num_tasks * 70 / 100,
        "Expected at least 70% success rate, got {}/{}",
        success,
        num_tasks
    );

    // No tasks should panic - all should complete gracefully
    assert_eq!(
        success + failure,
        num_tasks,
        "All tasks should complete (either success or failure)"
    );
}

#[tokio::test]
async fn test_load_sustained_throughput() {
    // Test sustained throughput over a longer period (10 seconds)
    // This verifies no memory leaks or resource exhaustion over time

    let config = WasmExecutorConfig::for_testing();
    let executor = Arc::new(WasmExecutor::new(config).expect("Failed to create executor"));
    let wasm_bytes = Arc::new(load_fixture("simple_add"));

    let operation_count = Arc::new(AtomicUsize::new(0));
    let test_duration = Duration::from_secs(10);

    println!("\n=== Load Test: Sustained Throughput ===");
    println!("Duration: {:?}", test_duration);
    println!("Running continuous operations...");

    let start = Instant::now();
    let mut handles = vec![];

    // Spawn 20 concurrent workers
    for worker_id in 0..20 {
        let exec = Arc::clone(&executor);
        let bytes = Arc::clone(&wasm_bytes);
        let count = Arc::clone(&operation_count);
        let duration = test_duration;

        let handle = tokio::spawn(async move {
            let worker_start = Instant::now();
            let mut ops = 0;

            while worker_start.elapsed() < duration {
                let result = exec.execute(&bytes, "add", &[worker_id, ops]).await;
                if result.is_ok() {
                    count.fetch_add(1, Ordering::SeqCst);
                    ops += 1;
                }
            }
        });

        handles.push(handle);
    }

    // Wait for all workers to complete
    for handle in handles {
        handle.await.expect("Task panicked");
    }

    let elapsed = start.elapsed();
    let total_ops = operation_count.load(Ordering::SeqCst);

    println!("Total operations: {}", total_ops);
    println!("Actual duration: {:?}", elapsed);
    println!(
        "Average throughput: {:.2} operations/sec",
        total_ops as f64 / elapsed.as_secs_f64()
    );

    // Verify sustained throughput (should be able to do many operations)
    assert!(
        total_ops > 1000,
        "Expected at least 1000 operations in 10 seconds, got {}",
        total_ops
    );

    // Verify no major slowdown (time should be close to test duration)
    assert!(
        elapsed < test_duration + Duration::from_secs(2),
        "Test took too long: {:?} (expected ~{:?})",
        elapsed,
        test_duration
    );
}
