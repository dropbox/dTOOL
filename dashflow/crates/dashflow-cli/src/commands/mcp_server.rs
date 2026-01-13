//! MCP Server - Expose DashFlow introspection data via HTTP
//!
//! Uses the unified MCP server from the core library, which provides:
//! - Module discovery endpoints (always available)
//! - Graph introspection endpoints (available when graph provided, returns 503 otherwise)
//!
//! ```bash
//! # Start the server in foreground
//! dashflow mcp-server --port 3200
//!
//! # Start the server in background (daemonized)
//! dashflow mcp-server --port 3200 --background
//!
//! # Stop the background server
//! dashflow mcp-server --stop
//!
//! # Module discovery endpoints (always available)
//! curl http://localhost:3200/modules                    # All modules
//! curl http://localhost:3200/modules/distillation       # Specific module
//! curl http://localhost:3200/search?q=distill           # Search
//! curl http://localhost:3200/health                     # Health check
//!
//! # Graph introspection endpoints (return 503 without --with-graph)
//! curl http://localhost:3200/mcp/about                  # Graph info
//! curl http://localhost:3200/mcp/architecture           # Graph structure
//! ```
//!
//! ## Auto-Start with direnv
//!
//! Add to `.envrc`:
//! ```bash
//! # Start MCP server when entering project directory
//! dashflow mcp-server --background 2>/dev/null || true
//! ```
//!
//! ## PID File Location
//!
//! Default: `.dashflow/mcp-server.pid`
//! Custom: `--pid-file /path/to/file.pid`

use anyhow::{Context, Result};
use clap::Args;
use dashflow::mcp_self_doc::UnifiedMcpServer;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Default PID file location
const DEFAULT_PID_FILE: &str = ".dashflow/mcp-server.pid";

#[derive(Args)]
pub struct McpServerArgs {
    /// Port to listen on
    #[arg(long, default_value = "3200")]
    port: u16,

    /// Path to dashflow/src directory (default: auto-detect)
    #[arg(long)]
    src_path: Option<PathBuf>,

    /// Run server in background (daemonized)
    #[arg(long)]
    background: bool,

    /// PID file for background server management
    #[arg(long, default_value = DEFAULT_PID_FILE)]
    pid_file: PathBuf,

    /// Stop a running background server
    #[arg(long)]
    stop: bool,

    /// Check if server is running
    #[arg(long)]
    status: bool,
}

pub async fn run(args: McpServerArgs) -> Result<()> {
    // Handle --stop
    if args.stop {
        return stop_server(&args.pid_file);
    }

    // Handle --status
    if args.status {
        return check_status(&args.pid_file);
    }

    // Handle --background
    if args.background {
        return start_background(&args);
    }

    // Normal foreground operation
    run_foreground(args).await
}

/// Stop a running background server
fn stop_server(pid_file: &PathBuf) -> Result<()> {
    if !pid_file.exists() {
        println!(
            "No PID file found at {}. Server may not be running.",
            pid_file.display()
        );
        return Ok(());
    }

    let pid_str = fs::read_to_string(pid_file).context("Failed to read PID file")?;
    let pid: u32 = pid_str.trim().parse().context("Invalid PID in file")?;

    // Check if process is running
    #[cfg(unix)]
    {
        let status = Command::new("kill").arg("-0").arg(pid.to_string()).status();

        if !status.as_ref().is_ok_and(|s| s.success()) {
            println!(
                "Process {} is not running. Cleaning up stale PID file.",
                pid
            );
            fs::remove_file(pid_file)?;
            return Ok(());
        }

        // Send SIGTERM
        let result = Command::new("kill")
            .arg(pid.to_string())
            .status()
            .context("Failed to send kill signal")?;

        if result.success() {
            println!("Stopped MCP server (PID: {})", pid);
            fs::remove_file(pid_file)?;
        } else {
            anyhow::bail!("Failed to stop server (PID: {})", pid);
        }
    }

    #[cfg(not(unix))]
    {
        // On Windows, use taskkill
        let result = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status()
            .context("Failed to stop server")?;

        if result.success() {
            println!("Stopped MCP server (PID: {})", pid);
            fs::remove_file(pid_file)?;
        } else {
            anyhow::bail!("Failed to stop server (PID: {})", pid);
        }
    }

    Ok(())
}

/// Check if server is running
fn check_status(pid_file: &PathBuf) -> Result<()> {
    if !pid_file.exists() {
        println!("MCP server is not running (no PID file)");
        return Ok(());
    }

    let pid_str = fs::read_to_string(pid_file).context("Failed to read PID file")?;
    let pid: u32 = pid_str.trim().parse().context("Invalid PID in file")?;

    #[cfg(unix)]
    {
        let status = Command::new("kill").arg("-0").arg(pid.to_string()).status();

        if status.is_ok_and(|s| s.success()) {
            println!("MCP server is running (PID: {})", pid);
        } else {
            println!("MCP server is not running (stale PID file)");
            fs::remove_file(pid_file)?;
        }
    }

    #[cfg(not(unix))]
    {
        // On Windows, check with tasklist
        let output = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid)])
            .output();

        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.contains(&pid.to_string()) {
                println!("MCP server is running (PID: {})", pid);
            } else {
                println!("MCP server is not running (stale PID file)");
                fs::remove_file(pid_file)?;
            }
        } else {
            println!("Could not check server status");
        }
    }

    Ok(())
}

/// Start server in background (daemonized)
fn start_background(args: &McpServerArgs) -> Result<()> {
    // Ensure .dashflow directory exists
    if let Some(parent) = args.pid_file.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).context("Failed to create .dashflow directory")?;
        }
    }

    // Check if already running
    if args.pid_file.exists() {
        let pid_str = fs::read_to_string(&args.pid_file)?;
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            #[cfg(unix)]
            {
                let status = Command::new("kill").arg("-0").arg(pid.to_string()).status();

                if status.is_ok_and(|s| s.success()) {
                    println!("MCP server already running (PID: {})", pid);
                    return Ok(());
                }
            }
        }
        // Stale PID file, remove it
        fs::remove_file(&args.pid_file)?;
    }

    // Get the current executable path
    let exe = std::env::current_exe().context("Failed to get current executable path")?;

    // Build command args (without --background to avoid infinite loop)
    let mut cmd_args = vec![
        "mcp-server".to_string(),
        "--port".to_string(),
        args.port.to_string(),
        "--pid-file".to_string(),
        args.pid_file.to_string_lossy().to_string(),
    ];

    if let Some(ref src) = args.src_path {
        cmd_args.push("--src-path".to_string());
        cmd_args.push(src.to_string_lossy().to_string());
    }

    // Spawn the process with detached stdio
    let child = Command::new(&exe)
        .args(&cmd_args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to spawn background server")?;

    let pid = child.id();

    // Write PID file
    fs::write(&args.pid_file, pid.to_string()).context("Failed to write PID file")?;

    println!("Started MCP server in background (PID: {})", pid);
    println!("Server running at http://localhost:{}", args.port);
    println!("Stop with: dashflow mcp-server --stop");
    println!("PID file: {}", args.pid_file.display());

    Ok(())
}

/// Run server in foreground (normal operation)
async fn run_foreground(args: McpServerArgs) -> Result<()> {
    // Write PID file even in foreground mode for status checks
    if let Some(parent) = args.pid_file.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::write(&args.pid_file, std::process::id().to_string())?;

    // Find the dashflow src directory
    let src_path = args.src_path.unwrap_or_else(|| {
        // Try to find it relative to the binary
        let candidates = vec![
            PathBuf::from("crates/dashflow/src"),
            PathBuf::from("../crates/dashflow/src"),
            PathBuf::from("../../crates/dashflow/src"),
        ];

        for candidate in candidates {
            if candidate.exists() {
                return candidate;
            }
        }

        // Fallback
        PathBuf::from("crates/dashflow/src")
    });

    println!("Discovering modules from: {}", src_path.display());
    let server = UnifiedMcpServer::discovery_only(&src_path);
    println!("Discovered {} modules", server.module_count());

    println!("Starting MCP server at http://0.0.0.0:{}", args.port);
    println!("\nModule Discovery Endpoints:");
    println!("  GET /modules              - List all modules");
    println!("  GET /modules/:name        - Get specific module");
    println!("  GET /search?q=<query>     - Search modules");
    println!("  GET /health               - Health check");
    println!("\nGraph Introspection Endpoints (503 without graph):");
    println!("  GET /mcp/about            - Graph overview");
    println!("  GET /mcp/capabilities     - Tools and capabilities");
    println!("  GET /mcp/architecture     - Graph structure");
    println!("  GET /mcp/nodes            - List all nodes");
    println!("  GET /mcp/edges            - List all edges");
    println!("\nPID file: {}", args.pid_file.display());

    // Cleanup PID file on exit
    let pid_file = args.pid_file.clone();
    let _cleanup = PidFileCleanup { pid_file };

    server
        .serve(args.port)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok(())
}

/// RAII guard to cleanup PID file on exit
struct PidFileCleanup {
    pid_file: PathBuf,
}

impl Drop for PidFileCleanup {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.pid_file);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Cli {
        #[command(flatten)]
        mcp: McpServerArgs,
    }

    #[test]
    fn test_mcp_server_args_defaults() {
        let cli = Cli::parse_from(["test"]);

        assert_eq!(cli.mcp.port, 3200);
        assert!(cli.mcp.src_path.is_none());
        assert!(!cli.mcp.background);
        assert_eq!(cli.mcp.pid_file.to_str().unwrap(), DEFAULT_PID_FILE);
        assert!(!cli.mcp.stop);
        assert!(!cli.mcp.status);
    }

    #[test]
    fn test_mcp_server_args_custom_port() {
        let cli = Cli::parse_from(["test", "--port", "8080"]);

        assert_eq!(cli.mcp.port, 8080);
    }

    #[test]
    fn test_mcp_server_args_background_mode() {
        let cli = Cli::parse_from(["test", "--background"]);

        assert!(cli.mcp.background);
    }

    #[test]
    fn test_mcp_server_args_stop_flag() {
        let cli = Cli::parse_from(["test", "--stop"]);

        assert!(cli.mcp.stop);
    }

    #[test]
    fn test_mcp_server_args_status_flag() {
        let cli = Cli::parse_from(["test", "--status"]);

        assert!(cli.mcp.status);
    }

    #[test]
    fn test_mcp_server_args_custom_pid_file() {
        let cli = Cli::parse_from(["test", "--pid-file", "/tmp/mcp.pid"]);

        assert_eq!(cli.mcp.pid_file.to_str().unwrap(), "/tmp/mcp.pid");
    }

    #[test]
    fn test_mcp_server_args_src_path() {
        let cli = Cli::parse_from(["test", "--src-path", "/custom/src"]);

        assert_eq!(
            cli.mcp.src_path.unwrap().to_str().unwrap(),
            "/custom/src"
        );
    }

    #[test]
    fn test_mcp_server_args_full_config() {
        let cli = Cli::parse_from([
            "test",
            "--port", "9000",
            "--src-path", "/src",
            "--pid-file", "/var/run/mcp.pid",
            "--background",
        ]);

        assert_eq!(cli.mcp.port, 9000);
        assert_eq!(cli.mcp.src_path.unwrap().to_str().unwrap(), "/src");
        assert_eq!(cli.mcp.pid_file.to_str().unwrap(), "/var/run/mcp.pid");
        assert!(cli.mcp.background);
    }
}
