//! Integration tests for sandbox enforcement
//!
//! These tests verify that the sandbox correctly restricts operations
//! based on the configured mode.
//!
//! Note: These tests require platform-specific sandbox support:
//! - macOS: Seatbelt (sandbox-exec)
//! - Linux: Landlock + seccomp

use codex_dashflow_sandbox::{SandboxExecutor, SandboxMode};
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper to check if sandbox is available on the current platform
fn sandbox_available() -> bool {
    SandboxExecutor::is_available()
}

// ============================================================================
// Read-Only Mode Tests
// ============================================================================

#[tokio::test]
async fn test_readonly_allows_echo() {
    if !sandbox_available() {
        eprintln!("Sandbox not available, skipping test");
        return;
    }

    let executor = SandboxExecutor::new(SandboxMode::ReadOnly, PathBuf::from("/tmp"));
    let result = executor.execute("echo 'hello world'").await;

    assert!(result.is_ok(), "echo should succeed in read-only mode");
    assert!(result.unwrap().contains("hello world"));
}

#[tokio::test]
async fn test_readonly_allows_cat() {
    if !sandbox_available() {
        eprintln!("Sandbox not available, skipping test");
        return;
    }

    let executor = SandboxExecutor::new(SandboxMode::ReadOnly, PathBuf::from("/tmp"));
    // Read /etc/passwd which should always exist
    let result = executor.execute("cat /etc/passwd | head -1").await;

    assert!(result.is_ok(), "cat should succeed in read-only mode");
}

#[tokio::test]
async fn test_readonly_blocks_file_write() {
    if !sandbox_available() {
        eprintln!("Sandbox not available, skipping test");
        return;
    }

    let executor = SandboxExecutor::new(SandboxMode::ReadOnly, PathBuf::from("/tmp"));
    // Try to create a file - should fail
    let result = executor
        .execute("touch /tmp/sandbox_readonly_test_should_fail")
        .await;

    assert!(
        result.is_err(),
        "touch should fail in read-only mode: {:?}",
        result
    );
}

#[tokio::test]
async fn test_readonly_blocks_mkdir() {
    if !sandbox_available() {
        eprintln!("Sandbox not available, skipping test");
        return;
    }

    let executor = SandboxExecutor::new(SandboxMode::ReadOnly, PathBuf::from("/tmp"));
    let result = executor
        .execute("mkdir /tmp/sandbox_readonly_mkdir_test")
        .await;

    assert!(
        result.is_err(),
        "mkdir should fail in read-only mode: {:?}",
        result
    );
}

#[tokio::test]
async fn test_readonly_blocks_rm() {
    if !sandbox_available() {
        eprintln!("Sandbox not available, skipping test");
        return;
    }

    let executor = SandboxExecutor::new(SandboxMode::ReadOnly, PathBuf::from("/tmp"));
    // Try to remove a system file (this would be blocked anyway, but sandbox should also block)
    let result = executor.execute("rm /etc/hosts 2>&1").await;

    // Should fail - either due to sandbox or permissions
    assert!(
        result.is_err(),
        "rm should fail in read-only mode: {:?}",
        result
    );
}

// ============================================================================
// Workspace-Write Mode Tests
// ============================================================================

#[tokio::test]
async fn test_workspace_write_allows_write_in_workspace() {
    if !sandbox_available() {
        eprintln!("Sandbox not available, skipping test");
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().to_path_buf();

    let executor = SandboxExecutor::new(SandboxMode::WorkspaceWrite, workspace.clone());
    let test_file = workspace.join("test_file.txt");
    let cmd = format!("echo 'test content' > {}", test_file.display());

    let result = executor.execute(&cmd).await;
    assert!(
        result.is_ok(),
        "writing in workspace should succeed: {:?}",
        result
    );

    // Verify file was created
    assert!(test_file.exists(), "test file should have been created");
}

#[tokio::test]
async fn test_workspace_write_allows_write_in_tmp() {
    if !sandbox_available() {
        eprintln!("Sandbox not available, skipping test");
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().to_path_buf();

    let executor = SandboxExecutor::new(SandboxMode::WorkspaceWrite, workspace);
    // /tmp should be writable in workspace-write mode
    let result = executor
        .execute("touch /tmp/sandbox_workspace_test_file && rm /tmp/sandbox_workspace_test_file")
        .await;

    assert!(result.is_ok(), "/tmp should be writable: {:?}", result);
}

#[tokio::test]
async fn test_workspace_write_blocks_write_outside_workspace() {
    if !sandbox_available() {
        eprintln!("Sandbox not available, skipping test");
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().to_path_buf();

    let executor = SandboxExecutor::new(SandboxMode::WorkspaceWrite, workspace);
    // Try to write outside workspace (not in /tmp either)
    let result = executor
        .execute("touch /var/sandbox_test_should_fail")
        .await;

    assert!(
        result.is_err(),
        "writing outside workspace should fail: {:?}",
        result
    );
}

#[tokio::test]
async fn test_workspace_write_allows_read_outside_workspace() {
    if !sandbox_available() {
        eprintln!("Sandbox not available, skipping test");
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().to_path_buf();

    let executor = SandboxExecutor::new(SandboxMode::WorkspaceWrite, workspace);
    // Reading outside workspace should still work
    let result = executor.execute("cat /etc/passwd | head -1").await;

    assert!(
        result.is_ok(),
        "reading outside workspace should succeed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_workspace_write_with_additional_writable_root() {
    if !sandbox_available() {
        eprintln!("Sandbox not available, skipping test");
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().to_path_buf();
    let extra_dir = TempDir::new().unwrap();
    let extra_path = extra_dir.path().to_path_buf();

    let executor = SandboxExecutor::new(SandboxMode::WorkspaceWrite, workspace)
        .with_writable_root(extra_path.clone());

    let test_file = extra_path.join("extra_test.txt");
    let cmd = format!("echo 'extra content' > {}", test_file.display());

    let result = executor.execute(&cmd).await;
    assert!(
        result.is_ok(),
        "writing in additional writable root should succeed: {:?}",
        result
    );
}

// ============================================================================
// Full Access Mode Tests
// ============================================================================

#[tokio::test]
async fn test_full_access_allows_all_operations() {
    // Note: This test doesn't require sandbox to be available since full-access mode
    // runs without restrictions
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().to_path_buf();

    let executor = SandboxExecutor::new(SandboxMode::DangerFullAccess, workspace.clone());
    let test_file = workspace.join("full_access_test.txt");
    let cmd = format!("echo 'test' > {}", test_file.display());

    let result = executor.execute(&cmd).await;
    assert!(
        result.is_ok(),
        "full access should allow writes: {:?}",
        result
    );
    assert!(test_file.exists());
}

// ============================================================================
// Network Restriction Tests
// ============================================================================

#[tokio::test]
async fn test_readonly_blocks_network() {
    if !sandbox_available() {
        eprintln!("Sandbox not available, skipping test");
        return;
    }

    let executor = SandboxExecutor::new(SandboxMode::ReadOnly, PathBuf::from("/tmp"));
    // Try to make a network connection - should fail
    // Using curl with a very short timeout to make the test fast
    let result = executor
        .execute("curl --connect-timeout 1 -s http://example.com 2>&1")
        .await;

    // The command itself might "succeed" (exit 0) but with an error message,
    // or it might fail outright. Either way, we should see evidence of blocking.
    match result {
        Ok(output) => {
            // If curl ran but network was blocked, we'd see an error in output
            // Note: On some systems curl might not be available
            if !output.contains("not found") && !output.contains("No such file") {
                assert!(
                    output.contains("denied")
                        || output.contains("Permission")
                        || output.contains("Connection refused")
                        || output.contains("Could not resolve")
                        || output.contains("Operation not permitted"),
                    "Expected network to be blocked, got: {}",
                    output
                );
            }
        }
        Err(_) => {
            // Command failed - network was blocked
        }
    }
}

#[tokio::test]
async fn test_workspace_write_blocks_network() {
    if !sandbox_available() {
        eprintln!("Sandbox not available, skipping test");
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().to_path_buf();

    let executor = SandboxExecutor::new(SandboxMode::WorkspaceWrite, workspace);
    let result = executor
        .execute("curl --connect-timeout 1 -s http://example.com 2>&1")
        .await;

    match result {
        Ok(output) => {
            if !output.contains("not found") && !output.contains("No such file") {
                assert!(
                    output.contains("denied")
                        || output.contains("Permission")
                        || output.contains("Connection refused")
                        || output.contains("Could not resolve")
                        || output.contains("Operation not permitted"),
                    "Expected network to be blocked, got: {}",
                    output
                );
            }
        }
        Err(_) => {
            // Command failed - network was blocked
        }
    }
}

// ============================================================================
// Mode Property Tests
// ============================================================================

#[test]
fn test_sandbox_mode_properties() {
    // ReadOnly
    assert!(!SandboxMode::ReadOnly.allows_write());
    assert!(!SandboxMode::ReadOnly.allows_network());
    assert!(!SandboxMode::ReadOnly.is_unrestricted());

    // WorkspaceWrite
    assert!(SandboxMode::WorkspaceWrite.allows_write());
    assert!(!SandboxMode::WorkspaceWrite.allows_network());
    assert!(!SandboxMode::WorkspaceWrite.is_unrestricted());

    // DangerFullAccess
    assert!(SandboxMode::DangerFullAccess.allows_write());
    assert!(SandboxMode::DangerFullAccess.allows_network());
    assert!(SandboxMode::DangerFullAccess.is_unrestricted());
}

#[test]
fn test_sandbox_mode_display() {
    assert_eq!(SandboxMode::ReadOnly.to_string(), "read-only");
    assert_eq!(SandboxMode::WorkspaceWrite.to_string(), "workspace-write");
    assert_eq!(SandboxMode::DangerFullAccess.to_string(), "full-access");
}

#[test]
fn test_sandbox_mode_parse() {
    use std::str::FromStr;

    assert_eq!(
        SandboxMode::from_str("read-only").unwrap(),
        SandboxMode::ReadOnly
    );
    assert_eq!(
        SandboxMode::from_str("readonly").unwrap(),
        SandboxMode::ReadOnly
    );
    assert_eq!(
        SandboxMode::from_str("workspace-write").unwrap(),
        SandboxMode::WorkspaceWrite
    );
    assert_eq!(
        SandboxMode::from_str("full-access").unwrap(),
        SandboxMode::DangerFullAccess
    );
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[tokio::test]
async fn test_command_failure_returns_error() {
    // This test doesn't need sandbox
    let executor = SandboxExecutor::new(SandboxMode::DangerFullAccess, PathBuf::from("/tmp"));
    let result = executor.execute("exit 1").await;

    assert!(result.is_err(), "exit 1 should return error");
}

#[tokio::test]
async fn test_command_with_stderr() {
    // This test doesn't need sandbox
    let executor = SandboxExecutor::new(SandboxMode::DangerFullAccess, PathBuf::from("/tmp"));
    let result = executor.execute("ls /nonexistent_path_12345").await;

    assert!(result.is_err(), "ls of nonexistent path should fail");
    if let Err(e) = result {
        let error_msg = e.to_string();
        assert!(
            error_msg.contains("No such file") || error_msg.contains("not exist"),
            "Error should mention missing file: {}",
            error_msg
        );
    }
}
