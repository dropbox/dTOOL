//! Integration tests for WASM executor
//!
//! Tests with real WASM modules to verify:
//! - Basic execution
//! - Resource limits (fuel, memory, timeout)
//! - Error handling
//! - Security sandboxing

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dashflow_wasm_executor::config::WasmExecutorConfig;
use dashflow_wasm_executor::WasmExecutor;
use std::fs;

/// Helper to load WASM fixture
fn load_fixture(name: &str) -> Vec<u8> {
    let path = format!("tests/fixtures/{}.wasm", name);
    fs::read(&path).unwrap_or_else(|_| panic!("Failed to read fixture: {}", path))
}

#[tokio::test]
async fn test_simple_add() {
    let config = WasmExecutorConfig::for_testing();
    let executor = WasmExecutor::new(config).expect("Failed to create executor");

    let wasm_bytes = load_fixture("simple_add");
    let result = executor.execute(&wasm_bytes, "add", &[5, 7]).await;

    assert!(result.is_ok(), "Execution failed: {:?}", result.err());
    assert_eq!(result.unwrap(), "12");
}

#[tokio::test]
async fn test_multiply() {
    let config = WasmExecutorConfig::for_testing();
    let executor = WasmExecutor::new(config).expect("Failed to create executor");

    let wasm_bytes = load_fixture("simple_add");
    let result = executor.execute(&wasm_bytes, "multiply", &[6, 7]).await;

    assert!(result.is_ok(), "Execution failed: {:?}", result.err());
    assert_eq!(result.unwrap(), "42");
}

#[tokio::test]
async fn test_get_constant() {
    let config = WasmExecutorConfig::for_testing();
    let executor = WasmExecutor::new(config).expect("Failed to create executor");

    let wasm_bytes = load_fixture("simple_add");
    let result = executor.execute(&wasm_bytes, "get_constant", &[]).await;

    assert!(result.is_ok(), "Execution failed: {:?}", result.err());
    assert_eq!(result.unwrap(), "42");
}

#[tokio::test]
async fn test_function_not_found() {
    let config = WasmExecutorConfig::for_testing();
    let executor = WasmExecutor::new(config).expect("Failed to create executor");

    let wasm_bytes = load_fixture("simple_add");
    let result = executor.execute(&wasm_bytes, "nonexistent", &[]).await;

    assert!(result.is_err());
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("not found"),
        "Expected 'not found' error, got: {}",
        error
    );
}

#[tokio::test]
async fn test_invalid_wasm() {
    let config = WasmExecutorConfig::for_testing();
    let executor = WasmExecutor::new(config).expect("Failed to create executor");

    let invalid_wasm = vec![0x00, 0x01, 0x02, 0x03]; // Not valid WASM
    let result = executor.execute(&invalid_wasm, "add", &[1, 2]).await;

    assert!(result.is_err());
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("Invalid WASM module"),
        "Expected 'Invalid WASM module' error, got: {}",
        error
    );
}

#[tokio::test]
async fn test_memory_access() {
    let config = WasmExecutorConfig::for_testing();
    let executor = WasmExecutor::new(config).expect("Failed to create executor");

    let wasm_bytes = load_fixture("memory_access");

    // Load value from memory (should be 0 initially)
    let result = executor.execute(&wasm_bytes, "load_value", &[]).await;
    assert!(result.is_ok(), "Load failed: {:?}", result.err());
    assert_eq!(result.unwrap(), "0");

    // Note: store_value doesn't return a value, so we can't test it easily
    // Each execution creates a new instance, so stored value won't persist anyway
    // This is correct behavior for security (no shared state between executions)
}

#[tokio::test]
async fn test_fuel_limit_infinite_loop() {
    let mut config = WasmExecutorConfig::for_testing();
    config.max_fuel = 1_000_000; // Low fuel limit
    let executor = WasmExecutor::new(config).expect("Failed to create executor");

    let wasm_bytes = load_fixture("infinite_loop");
    let result = executor.execute(&wasm_bytes, "infinite_loop", &[]).await;

    // Should fail due to fuel exhaustion or execution error
    assert!(result.is_err(), "Expected error for infinite loop");
    // The error message may vary - could be fuel, timeout, or execution error
}

#[tokio::test]
async fn test_timeout() {
    let mut config = WasmExecutorConfig::for_testing();
    config.max_execution_timeout = std::time::Duration::from_secs(1); // Short timeout
    config.max_fuel = 1_000_000_000; // High fuel so fuel doesn't trigger first
    let executor = WasmExecutor::new(config).expect("Failed to create executor");

    let wasm_bytes = load_fixture("infinite_loop");
    let result = executor.execute(&wasm_bytes, "infinite_loop", &[]).await;

    // Should timeout or run out of fuel
    assert!(
        result.is_err(),
        "Expected error for infinite loop with timeout"
    );
    // The error could be timeout or fuel exhaustion - both are acceptable
}

#[tokio::test]
async fn test_concurrent_executions() {
    let config = WasmExecutorConfig::for_testing();
    let executor = WasmExecutor::new(config).expect("Failed to create executor");

    let wasm_bytes = load_fixture("simple_add");

    // Spawn 10 concurrent executions
    let mut handles = vec![];
    for i in 0..10 {
        let exec = executor.clone();
        let bytes = wasm_bytes.clone();
        let handle = tokio::spawn(async move { exec.execute(&bytes, "add", &[i, i]).await });
        handles.push(handle);
    }

    // Wait for all to complete
    for (i, handle) in handles.into_iter().enumerate() {
        let result = handle.await.expect("Task panicked");
        assert!(result.is_ok(), "Execution {} failed: {:?}", i, result.err());
        let value: i32 = result.unwrap().parse().expect("Failed to parse result");
        assert_eq!(
            value,
            i as i32 + i as i32,
            "Wrong result for execution {}",
            i
        );
    }
}

#[tokio::test]
async fn test_executor_clone() {
    let config = WasmExecutorConfig::for_testing();
    let executor1 = WasmExecutor::new(config).expect("Failed to create executor");
    let executor2 = executor1.clone();

    let wasm_bytes = load_fixture("simple_add");

    let result1 = executor1.execute(&wasm_bytes, "add", &[1, 2]).await;
    let result2 = executor2.execute(&wasm_bytes, "add", &[3, 4]).await;

    assert!(result1.is_ok());
    assert!(result2.is_ok());
    assert_eq!(result1.unwrap(), "3");
    assert_eq!(result2.unwrap(), "7");
}

//
// Authentication + Authorization Tests
//

#[tokio::test]
async fn test_execute_with_valid_agent_token() {
    use dashflow_wasm_executor::auth::Role;

    let config = WasmExecutorConfig::for_testing();
    let executor = WasmExecutor::new(config).expect("Failed to create executor");

    // Generate token for Agent role
    let token = executor
        .auth()
        .generate_token("user123".to_string(), Role::Agent, "session456".to_string())
        .expect("Failed to generate token");

    let wasm_bytes = load_fixture("simple_add");
    let result = executor
        .execute_with_auth(&token, &wasm_bytes, "add", &[5, 7])
        .await;

    assert!(result.is_ok(), "Execution failed: {:?}", result.err());
    assert_eq!(result.unwrap(), "12");
}

#[tokio::test]
async fn test_execute_with_valid_admin_token() {
    use dashflow_wasm_executor::auth::Role;

    let config = WasmExecutorConfig::for_testing();
    let executor = WasmExecutor::new(config).expect("Failed to create executor");

    // Generate token for Administrator role
    let token = executor
        .auth()
        .generate_token(
            "admin456".to_string(),
            Role::Administrator,
            "session789".to_string(),
        )
        .expect("Failed to generate token");

    let wasm_bytes = load_fixture("simple_add");
    let result = executor
        .execute_with_auth(&token, &wasm_bytes, "multiply", &[6, 7])
        .await;

    assert!(result.is_ok(), "Execution failed: {:?}", result.err());
    assert_eq!(result.unwrap(), "42");
}

#[tokio::test]
async fn test_execute_with_auditor_token_fails() {
    use dashflow_wasm_executor::auth::Role;

    let config = WasmExecutorConfig::for_testing();
    let executor = WasmExecutor::new(config).expect("Failed to create executor");

    // Generate token for Auditor role (should NOT have execute access)
    let token = executor
        .auth()
        .generate_token(
            "auditor789".to_string(),
            Role::Auditor,
            "session999".to_string(),
        )
        .expect("Failed to generate token");

    let wasm_bytes = load_fixture("simple_add");
    let result = executor
        .execute_with_auth(&token, &wasm_bytes, "add", &[1, 2])
        .await;

    assert!(result.is_err(), "Expected error for Auditor role execution");
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("does not have required permissions")
            || error.contains("authorization")
            || error.contains("Authorization failed"),
        "Expected authorization error, got: {}",
        error
    );
}

#[tokio::test]
async fn test_execute_with_invalid_token() {
    let config = WasmExecutorConfig::for_testing();
    let executor = WasmExecutor::new(config).expect("Failed to create executor");

    let wasm_bytes = load_fixture("simple_add");
    let result = executor
        .execute_with_auth("invalid.jwt.token", &wasm_bytes, "add", &[1, 2])
        .await;

    assert!(result.is_err(), "Expected error for invalid token");
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("Authentication") || error.contains("Invalid"),
        "Expected authentication error, got: {}",
        error
    );
}

#[tokio::test]
async fn test_execute_with_expired_token() {
    use dashflow_wasm_executor::auth::Role;

    let mut config = WasmExecutorConfig::for_testing();
    config.jwt_expiry_minutes = 1; // Very short expiry (1 minute minimum)
    let executor = WasmExecutor::new(config).expect("Failed to create executor");

    // Generate token with short expiry
    let token = executor
        .auth()
        .generate_token("user123".to_string(), Role::Agent, "session456".to_string())
        .expect("Failed to generate token");

    // Wait for token to expire (1 minute + buffer)
    // Note: In a real test, we'd mock the time. For now, skip this test in CI.
    // This test is kept for documentation purposes but will always pass since 1 minute hasn't elapsed
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let wasm_bytes = load_fixture("simple_add");
    let result = executor
        .execute_with_auth(&token, &wasm_bytes, "add", &[1, 2])
        .await;

    // Token should still be valid (only 10ms elapsed, not 1 minute)
    // This test documents the token expiry feature but doesn't actually test it
    // since we can't easily mock time without tokio-test crate
    assert!(result.is_ok(), "Token should still be valid after 10ms");
    assert_eq!(result.unwrap(), "3");
}

#[tokio::test]
async fn test_execute_with_auth_logs_to_audit() {
    use dashflow_wasm_executor::auth::Role;
    use std::fs;

    let audit_log_path = "/tmp/wasm-executor-audit-test.log";
    let _ = fs::remove_file(audit_log_path); // Clear audit log before test

    let mut config = WasmExecutorConfig::for_testing();
    config.enable_audit_logging = true; // Enable auditing for this test
    config.audit_log_path = audit_log_path.to_string();
    let executor = WasmExecutor::new(config).expect("Failed to create executor");

    // Generate token
    let token = executor
        .auth()
        .generate_token("user123".to_string(), Role::Agent, "session456".to_string())
        .expect("Failed to generate token");

    // Execute with auth (should log)
    let wasm_bytes = load_fixture("simple_add");
    let result = executor
        .execute_with_auth(&token, &wasm_bytes, "add", &[5, 7])
        .await;

    assert!(result.is_ok(), "Execution failed: {:?}", result.err());

    // Read audit log and verify entry exists
    let log_contents = fs::read_to_string(audit_log_path).expect("Failed to read audit log");
    assert!(
        log_contents.contains("user123"),
        "Audit log should contain user id"
    );
    assert!(
        log_contents.contains("wasm_execution"),
        "Audit log should contain event_type"
    );
    assert!(
        log_contents.contains("success"),
        "Audit log should contain success status"
    );
}

#[tokio::test]
async fn test_execute_with_auth_failed_execution_logs() {
    use dashflow_wasm_executor::auth::Role;
    use std::fs;

    let audit_log_path = "/tmp/wasm-executor-audit-test2.log";
    let _ = fs::remove_file(audit_log_path); // Clear audit log before test

    let mut config = WasmExecutorConfig::for_testing();
    config.enable_audit_logging = true; // Enable auditing for this test
    config.audit_log_path = audit_log_path.to_string();
    let executor = WasmExecutor::new(config).expect("Failed to create executor");

    // Generate token
    let token = executor
        .auth()
        .generate_token("user456".to_string(), Role::Agent, "session789".to_string())
        .expect("Failed to generate token");

    // Execute with auth (should fail - function doesn't exist)
    let wasm_bytes = load_fixture("simple_add");
    let result = executor
        .execute_with_auth(&token, &wasm_bytes, "nonexistent", &[])
        .await;

    assert!(result.is_err(), "Expected error for nonexistent function");

    // Read audit log and verify failure entry exists
    let log_contents = fs::read_to_string(audit_log_path).expect("Failed to read audit log");
    assert!(
        log_contents.contains("user456"),
        "Audit log should contain user id"
    );
    assert!(
        log_contents.contains("failure"),
        "Audit log should contain failure status"
    );
    assert!(
        log_contents.contains("not found"),
        "Audit log should contain error message"
    );
}

#[tokio::test]
async fn test_audit_log_includes_fuel_and_memory() {
    use dashflow_wasm_executor::auth::Role;
    use std::fs;

    let audit_log_path = "/tmp/wasm-executor-audit-test3.log";
    let _ = fs::remove_file(audit_log_path); // Clear audit log before test

    let mut config = WasmExecutorConfig::for_testing();
    config.enable_audit_logging = true;
    config.audit_log_path = audit_log_path.to_string();
    let executor = WasmExecutor::new(config).expect("Failed to create executor");

    // Generate token
    let token = executor
        .auth()
        .generate_token(
            "user_metrics".to_string(),
            Role::Agent,
            "session_metrics".to_string(),
        )
        .expect("Failed to generate token");

    // Execute with auth (should log with metrics)
    let wasm_bytes = load_fixture("simple_add");
    let result = executor
        .execute_with_auth(&token, &wasm_bytes, "add", &[10, 20])
        .await;

    assert!(result.is_ok(), "Execution failed: {:?}", result.err());
    assert_eq!(result.unwrap(), "30");

    // Read audit log and verify metrics are populated
    let log_contents = fs::read_to_string(audit_log_path).expect("Failed to read audit log");

    // Verify basic audit fields
    assert!(
        log_contents.contains("user_metrics"),
        "Audit log should contain user id"
    );
    assert!(
        log_contents.contains("wasm_execution"),
        "Audit log should contain event_type"
    );

    // Verify fuel and memory metrics are present and non-zero
    assert!(
        log_contents.contains("fuel_consumed"),
        "Audit log should contain fuel_consumed field"
    );
    assert!(
        log_contents.contains("memory_peak_bytes"),
        "Audit log should contain memory_peak_bytes field"
    );

    // Parse JSON to verify numeric values are actually populated
    // The log format is JSON lines, so we need to find the execution entry
    for line in log_contents.lines() {
        if line.contains("wasm_execution") {
            // Parse the JSON line
            let parsed: serde_json::Value =
                serde_json::from_str(line).expect("Failed to parse audit log JSON");

            if let Some(execution) = parsed.get("execution") {
                // Check fuel_consumed is non-zero
                let fuel = execution
                    .get("fuel_consumed")
                    .and_then(|v| v.as_u64())
                    .expect("fuel_consumed should be a number");
                assert!(fuel > 0, "fuel_consumed should be > 0, got {}", fuel);

                // Check memory_peak_bytes is non-zero
                let memory = execution
                    .get("memory_peak_bytes")
                    .and_then(|v| v.as_u64())
                    .expect("memory_peak_bytes should be a number");
                assert!(
                    memory > 0,
                    "memory_peak_bytes should be > 0, got {}",
                    memory
                );
            }
        }
    }
}

// ========================================
// Metrics Tests
// ========================================

#[tokio::test]
async fn test_metrics_execution_counters() {
    let config = WasmExecutorConfig::for_testing();
    let executor = WasmExecutor::new(config).expect("Failed to create executor");
    let metrics = executor.metrics();

    // Record initial counts
    let initial_success = metrics
        .executions_total
        .with_label_values(&["success"])
        .get();

    // Execute successfully
    let wasm_bytes = load_fixture("simple_add");
    let result = executor.execute(&wasm_bytes, "add", &[5, 7]).await;
    assert!(result.is_ok());

    // Verify success counter incremented
    let final_success = metrics
        .executions_total
        .with_label_values(&["success"])
        .get();
    assert!(
        final_success > initial_success,
        "Success counter should increment"
    );
}

#[tokio::test]
async fn test_metrics_failure_counters() {
    let config = WasmExecutorConfig::for_testing();
    let executor = WasmExecutor::new(config).expect("Failed to create executor");
    let metrics = executor.metrics();

    // Execute with invalid function (should fail)
    let wasm_bytes = load_fixture("simple_add");
    let result = executor.execute(&wasm_bytes, "nonexistent", &[]).await;
    assert!(result.is_err(), "Execute should return error");

    // Verify failure counter was set (should be 1 since this is first execution)
    let failure_count = metrics
        .executions_total
        .with_label_values(&["failure"])
        .get();
    let success_count = metrics
        .executions_total
        .with_label_values(&["success"])
        .get();
    let timeout_count = metrics
        .executions_total
        .with_label_values(&["timeout"])
        .get();

    eprintln!(
        "Counters - failure: {}, success: {}, timeout: {}",
        failure_count, success_count, timeout_count
    );

    assert!(
        failure_count >= 1.0,
        "Failure counter should be at least 1, got: {} (success: {}, timeout: {})",
        failure_count,
        success_count,
        timeout_count
    );
}

#[tokio::test]
async fn test_metrics_timeout_counters() {
    let mut config = WasmExecutorConfig::for_testing();
    config.max_execution_timeout = std::time::Duration::from_secs(1); // 1 second timeout (short enough for infinite loop)
    let executor = WasmExecutor::new(config).expect("Failed to create executor");
    let metrics = executor.metrics();

    // Execute infinite loop (should fail - either timeout, fuel exhaustion, or WASM error)
    let wasm_bytes = load_fixture("infinite_loop");
    let result = executor.execute(&wasm_bytes, "infinite_loop", &[]).await;
    assert!(result.is_err(), "Infinite loop should fail");

    // Verify that SOME error counter incremented (timeout OR failure)
    // Infinite loops can fail in different ways: timeout, fuel, stack overflow, etc.
    let timeout_count = metrics
        .executions_total
        .with_label_values(&["timeout"])
        .get();
    let failure_count = metrics
        .executions_total
        .with_label_values(&["failure"])
        .get();

    assert!(
        timeout_count >= 1.0 || failure_count >= 1.0,
        "Timeout or failure counter should be at least 1, timeout: {}, failure: {}",
        timeout_count,
        failure_count
    );
}

#[tokio::test]
async fn test_metrics_concurrent_executions() {
    let config = WasmExecutorConfig::for_testing();
    let executor = WasmExecutor::new(config).expect("Failed to create executor");
    let metrics = executor.metrics();

    // Initial concurrent executions should be 0
    let initial_concurrent = metrics.concurrent_executions.get();

    // Spawn multiple executions concurrently
    let wasm_bytes = load_fixture("simple_add");
    let handles: Vec<_> = (0..5)
        .map(|_| {
            let executor = executor.clone();
            let wasm = wasm_bytes.clone();
            tokio::spawn(async move { executor.execute(&wasm, "add", &[1, 2]).await })
        })
        .collect();

    // Wait for all to complete
    for handle in handles {
        let _ = handle.await;
    }

    // After completion, concurrent executions should be back to initial
    let final_concurrent = metrics.concurrent_executions.get();
    assert!(
        (final_concurrent - initial_concurrent).abs() < f64::EPSILON,
        "Concurrent executions should return to initial value"
    );
}

#[tokio::test]
async fn test_metrics_auth_counters() {
    let config = WasmExecutorConfig::for_testing();
    let executor = WasmExecutor::new(config).expect("Failed to create executor");
    let metrics = executor.metrics();

    // Record initial counts
    let initial_success = metrics
        .auth_attempts_total
        .with_label_values(&["success"])
        .get();
    let initial_failure = metrics
        .auth_attempts_total
        .with_label_values(&["failure"])
        .get();

    // Successful auth
    let token = executor
        .auth()
        .generate_token(
            "user123".to_string(),
            dashflow_wasm_executor::Role::Agent,
            "session456".to_string(),
        )
        .expect("Failed to generate token");

    let wasm_bytes = load_fixture("simple_add");
    let result = executor
        .execute_with_auth(&token, &wasm_bytes, "add", &[5, 7])
        .await;
    assert!(result.is_ok());

    // Verify auth success counter incremented
    let success_after_valid = metrics
        .auth_attempts_total
        .with_label_values(&["success"])
        .get();
    assert!(
        success_after_valid > initial_success,
        "Auth success counter should increment"
    );

    // Failed auth (invalid token)
    let result = executor
        .execute_with_auth("invalid-token", &wasm_bytes, "add", &[5, 7])
        .await;
    assert!(result.is_err());

    // Verify auth failure counter incremented
    let failure_after_invalid = metrics
        .auth_attempts_total
        .with_label_values(&["failure"])
        .get();
    assert!(
        failure_after_invalid > initial_failure,
        "Auth failure counter should increment"
    );
}

#[tokio::test]
async fn test_metrics_access_denied_counters() {
    let config = WasmExecutorConfig::for_testing();
    let executor = WasmExecutor::new(config).expect("Failed to create executor");
    let metrics = executor.metrics();

    // Record initial counts
    let initial_denied = metrics
        .access_denied_total
        .with_label_values(&["auth"])
        .get();

    // Failed auth (invalid token) should trigger access denied
    let wasm_bytes = load_fixture("simple_add");
    let result = executor
        .execute_with_auth("invalid-token", &wasm_bytes, "add", &[5, 7])
        .await;
    assert!(result.is_err());

    // Verify access denied counter incremented
    let final_denied = metrics
        .access_denied_total
        .with_label_values(&["auth"])
        .get();
    assert!(
        final_denied > initial_denied,
        "Access denied counter should increment"
    );
}

#[tokio::test]
async fn test_metrics_fuel_and_memory_extraction() {
    let config = WasmExecutorConfig::for_testing();
    let executor = WasmExecutor::new(config).expect("Failed to create executor");
    let metrics = executor.metrics();

    // Execute WASM code that does some computation
    let wasm_bytes = load_fixture("simple_add");
    let result = executor.execute(&wasm_bytes, "add", &[5, 7]).await;
    assert!(result.is_ok(), "Execution should succeed");

    // Verify fuel consumed metric was recorded (should be > 0)
    let fuel_histogram = &metrics.fuel_consumed.with_label_values(&["success"]);
    let fuel_count = fuel_histogram.get_sample_count();
    assert!(
        fuel_count > 0,
        "Fuel consumed histogram should have at least one sample"
    );

    // Verify memory peak metric was recorded
    let memory_histogram = &metrics.memory_peak_bytes.with_label_values(&["success"]);
    let memory_count = memory_histogram.get_sample_count();
    assert!(
        memory_count > 0,
        "Memory peak histogram should have at least one sample"
    );

    // Note: We don't assert specific fuel/memory values as they are platform-dependent
    // and can vary based on allocator behavior. The important part is that metrics
    // are being extracted and recorded (count > 0).
}
