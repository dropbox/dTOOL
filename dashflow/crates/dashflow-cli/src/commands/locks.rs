// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! CLI commands for parallel AI development lock management.
//!
//! Provides commands to list, acquire, release, and manage crate/module locks
//! for coordinating multiple AI workers on the same codebase.

use crate::output::{create_table, print_error, print_info, print_success, print_warning};
use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use colored::Colorize;
use dashflow::parallel::{Lock, LockError, LockManager, LockStatus};

/// Manage parallel AI development locks
#[derive(Args)]
pub struct LocksArgs {
    #[command(subcommand)]
    pub command: LocksCommand,
}

#[derive(Subcommand)]
pub enum LocksCommand {
    /// List all locks (active and expired)
    List(ListArgs),

    /// Acquire a lock on a scope
    Acquire(AcquireArgs),

    /// Release a lock you own
    Release(ReleaseArgs),

    /// Force release a lock (for stale lock cleanup)
    ForceRelease(ForceReleaseArgs),

    /// Show locks owned by a specific worker
    Mine(MineArgs),

    /// Clean up all expired locks
    Cleanup(CleanupArgs),

    /// Show lock status for a specific scope
    Status(StatusArgs),
}

/// List all locks
#[derive(Args)]
pub struct ListArgs {
    /// Show only active (non-expired) locks
    #[arg(long)]
    active: bool,

    /// Show only expired locks
    #[arg(long)]
    expired: bool,

    /// Path to locks directory (default: .dashflow/locks)
    #[arg(long)]
    locks_dir: Option<String>,
}

/// Acquire a lock
#[derive(Args)]
pub struct AcquireArgs {
    /// Scope to lock (e.g., "dashflow.optimize" or "dashflow-openai")
    pub scope: String,

    /// Worker ID (e.g., "claude-abc123")
    #[arg(short, long)]
    pub worker_id: String,

    /// Purpose/description of the work being done
    #[arg(short, long)]
    pub purpose: String,

    /// Lock duration in seconds (default: 3600 = 1 hour)
    #[arg(short, long)]
    duration: Option<i64>,

    /// Path to locks directory (default: .dashflow/locks)
    #[arg(long)]
    locks_dir: Option<String>,
}

/// Release a lock
#[derive(Args)]
pub struct ReleaseArgs {
    /// Scope to release
    pub scope: String,

    /// Worker ID (must match lock owner)
    #[arg(short, long)]
    pub worker_id: String,

    /// Path to locks directory (default: .dashflow/locks)
    #[arg(long)]
    locks_dir: Option<String>,
}

/// Force release a lock
#[derive(Args)]
pub struct ForceReleaseArgs {
    /// Scope to force release
    pub scope: String,

    /// Path to locks directory (default: .dashflow/locks)
    #[arg(long)]
    locks_dir: Option<String>,
}

/// Show locks for a worker
#[derive(Args)]
pub struct MineArgs {
    /// Worker ID to filter by
    #[arg(short, long)]
    pub worker_id: String,

    /// Path to locks directory (default: .dashflow/locks)
    #[arg(long)]
    locks_dir: Option<String>,
}

/// Clean up expired locks
#[derive(Args)]
pub struct CleanupArgs {
    /// Path to locks directory (default: .dashflow/locks)
    #[arg(long)]
    locks_dir: Option<String>,
}

/// Check status of a specific scope
#[derive(Args)]
pub struct StatusArgs {
    /// Scope to check
    pub scope: String,

    /// Path to locks directory (default: .dashflow/locks)
    #[arg(long)]
    locks_dir: Option<String>,
}

pub async fn run(args: LocksArgs) -> Result<()> {
    match args.command {
        LocksCommand::List(list_args) => run_list(list_args).await,
        LocksCommand::Acquire(acquire_args) => run_acquire(acquire_args).await,
        LocksCommand::Release(release_args) => run_release(release_args).await,
        LocksCommand::ForceRelease(force_args) => run_force_release(force_args).await,
        LocksCommand::Mine(mine_args) => run_mine(mine_args).await,
        LocksCommand::Cleanup(cleanup_args) => run_cleanup(cleanup_args).await,
        LocksCommand::Status(status_args) => run_status(status_args).await,
    }
}

fn get_lock_manager(locks_dir: Option<&str>) -> Result<LockManager> {
    match locks_dir {
        Some(dir) => LockManager::new(dir).context("Failed to initialize lock manager"),
        None => LockManager::default_location().context("Failed to initialize lock manager"),
    }
}

async fn run_list(args: ListArgs) -> Result<()> {
    let manager = get_lock_manager(args.locks_dir.as_deref())?;
    let summary = manager.summary().context("Failed to get lock summary")?;

    if summary.active_count == 0 && summary.expired_count == 0 {
        print_info("No locks found.");
        return Ok(());
    }

    // Filter based on flags
    let show_active = !args.expired || args.active;
    let show_expired = !args.active || args.expired;

    if show_active && !summary.active.is_empty() {
        println!();
        println!("{}", "Active Locks".bright_cyan().bold());
        println!("{}", "═".repeat(80).bright_cyan());
        print_locks_table(&summary.active);
    }

    if show_expired && !summary.expired.is_empty() {
        println!();
        println!("{}", "Expired Locks".bright_yellow().bold());
        println!("{}", "═".repeat(80).bright_yellow());
        print_locks_table(&summary.expired);
        println!();
        print_info(&format!(
            "Run 'dashflow locks cleanup' to remove {} expired lock(s).",
            summary.expired_count
        ));
    }

    // Summary line
    println!();
    println!(
        "Total: {} active, {} expired",
        summary.active_count.to_string().bright_green(),
        summary.expired_count.to_string().bright_yellow()
    );

    Ok(())
}

fn print_locks_table(locks: &[Lock]) {
    let mut table = create_table();
    table.set_header(vec!["Scope", "Worker", "Purpose", "Expires In"]);

    for lock in locks {
        let time_remaining = if let Some(remaining) = lock.time_remaining() {
            format_time_remaining(remaining)
        } else {
            "EXPIRED".bright_red().to_string()
        };

        table.add_row(vec![
            lock.scope.name().to_string(),
            lock.worker_id.clone(),
            truncate_string(&lock.purpose, 30),
            time_remaining,
        ]);
    }

    println!("{table}");
}

fn format_time_remaining(duration: chrono::Duration) -> String {
    let secs = duration.num_seconds();
    if secs < 60 {
        format!("{}s", secs).bright_yellow().to_string()
    } else if secs < 3600 {
        format!("{}m", secs / 60).bright_green().to_string()
    } else {
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        format!("{}h {}m", hours, mins).bright_green().to_string()
    }
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

async fn run_acquire(args: AcquireArgs) -> Result<()> {
    let manager = get_lock_manager(args.locks_dir.as_deref())?;

    match manager.acquire_with_duration(&args.scope, &args.worker_id, &args.purpose, args.duration)
    {
        Ok(lock) => {
            print_success(&format!(
                "Acquired lock on '{}' for worker '{}'",
                args.scope.bright_cyan(),
                args.worker_id.bright_green()
            ));

            let time_remaining = lock
                .time_remaining()
                .map(format_time_remaining)
                .unwrap_or_else(|| "N/A".to_string());
            println!("  Purpose:    {}", lock.purpose);
            println!("  Expires in: {}", time_remaining);
            Ok(())
        }
        Err(LockError::AlreadyLocked {
            scope,
            worker_id,
            expires_at,
        }) => {
            print_error(&format!(
                "Scope '{}' is already locked by '{}' until {}",
                scope.bright_cyan(),
                worker_id.bright_red(),
                expires_at.format("%Y-%m-%d %H:%M:%S UTC")
            ));
            std::process::exit(1);
        }
        Err(e) => Err(e.into()),
    }
}

async fn run_release(args: ReleaseArgs) -> Result<()> {
    let manager = get_lock_manager(args.locks_dir.as_deref())?;

    match manager.release(&args.scope, &args.worker_id) {
        Ok(()) => {
            print_success(&format!("Released lock on '{}'", args.scope.bright_cyan()));
            Ok(())
        }
        Err(LockError::NotLocked(scope)) => {
            print_error(&format!(
                "No lock exists for scope '{}'",
                scope.bright_cyan()
            ));
            std::process::exit(1);
        }
        Err(LockError::NotOwner {
            scope,
            owner,
            requestor,
        }) => {
            print_error(&format!(
                "Lock for '{}' is owned by '{}', not '{}'",
                scope.bright_cyan(),
                owner.bright_red(),
                requestor.bright_yellow()
            ));
            std::process::exit(1);
        }
        Err(e) => Err(e.into()),
    }
}

async fn run_force_release(args: ForceReleaseArgs) -> Result<()> {
    let manager = get_lock_manager(args.locks_dir.as_deref())?;

    // First check if the lock exists and show what we're releasing
    match manager.status(&args.scope) {
        Ok(LockStatus::Locked(lock)) => {
            print_warning(&format!(
                "Force releasing active lock owned by '{}'",
                lock.worker_id.bright_yellow()
            ));
        }
        Ok(LockStatus::Expired(lock)) => {
            print_info(&format!(
                "Releasing expired lock from '{}'",
                lock.worker_id.bright_yellow()
            ));
        }
        Ok(LockStatus::Unlocked) => {
            print_error(&format!(
                "No lock exists for scope '{}'",
                args.scope.bright_cyan()
            ));
            std::process::exit(1);
        }
        Err(e) => return Err(e.into()),
    }

    match manager.force_release(&args.scope) {
        Ok(()) => {
            print_success(&format!(
                "Force released lock on '{}'",
                args.scope.bright_cyan()
            ));
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

async fn run_mine(args: MineArgs) -> Result<()> {
    let manager = get_lock_manager(args.locks_dir.as_deref())?;
    let locks = manager
        .list_by_worker(&args.worker_id)
        .context("Failed to list locks")?;

    if locks.is_empty() {
        print_info(&format!(
            "No locks found for worker '{}'",
            args.worker_id.bright_cyan()
        ));
        return Ok(());
    }

    println!();
    println!(
        "{} {}",
        "Locks for worker:".bright_cyan().bold(),
        args.worker_id.bright_green()
    );
    println!("{}", "═".repeat(80).bright_cyan());

    print_locks_table(&locks);

    // Count active vs expired
    let active_count = locks.iter().filter(|l| !l.is_expired()).count();
    let expired_count = locks.len() - active_count;

    println!();
    println!(
        "Total: {} active, {} expired",
        active_count.to_string().bright_green(),
        expired_count.to_string().bright_yellow()
    );

    Ok(())
}

async fn run_cleanup(args: CleanupArgs) -> Result<()> {
    let manager = get_lock_manager(args.locks_dir.as_deref())?;

    let count = manager
        .cleanup_expired()
        .context("Failed to cleanup expired locks")?;

    if count == 0 {
        print_info("No expired locks to clean up.");
    } else {
        print_success(&format!("Cleaned up {} expired lock(s).", count));
    }

    Ok(())
}

async fn run_status(args: StatusArgs) -> Result<()> {
    let manager = get_lock_manager(args.locks_dir.as_deref())?;

    match manager.status(&args.scope) {
        Ok(LockStatus::Unlocked) => {
            println!(
                "{} {} is {}",
                "ℹ".bright_blue().bold(),
                args.scope.bright_cyan(),
                "UNLOCKED".bright_green().bold()
            );
        }
        Ok(LockStatus::Locked(lock)) => {
            println!(
                "{} {} is {}",
                "ℹ".bright_blue().bold(),
                args.scope.bright_cyan(),
                "LOCKED".bright_red().bold()
            );
            println!("  Worker:     {}", lock.worker_id.bright_yellow());
            println!("  Purpose:    {}", lock.purpose);
            println!(
                "  Acquired:   {}",
                lock.acquired_at.format("%Y-%m-%d %H:%M:%S UTC")
            );
            println!(
                "  Expires:    {}",
                lock.expires_at.format("%Y-%m-%d %H:%M:%S UTC")
            );
            if let Some(remaining) = lock.time_remaining() {
                println!("  Remaining:  {}", format_time_remaining(remaining));
            }
            if !lock.files_touched.is_empty() {
                println!("  Files:");
                for file in &lock.files_touched {
                    println!("    - {}", file);
                }
            }
        }
        Ok(LockStatus::Expired(lock)) => {
            println!(
                "{} {} is {} (owned by '{}')",
                "ℹ".bright_blue().bold(),
                args.scope.bright_cyan(),
                "EXPIRED".bright_yellow().bold(),
                lock.worker_id.bright_yellow()
            );
            println!("  Purpose:    {}", lock.purpose);
            println!(
                "  Expired at: {}",
                lock.expires_at.format("%Y-%m-%d %H:%M:%S UTC")
            );
            print_info("This lock can be acquired by any worker or cleaned up.");
        }
        Err(e) => {
            print_error(&format!("Failed to check lock status: {}", e));
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, String) {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_string_lossy().to_string();
        (temp_dir, path)
    }

    #[test]
    fn test_format_time_remaining() {
        assert!(format_time_remaining(chrono::Duration::seconds(30)).contains("30s"));
        assert!(format_time_remaining(chrono::Duration::seconds(120)).contains("2m"));
        assert!(format_time_remaining(chrono::Duration::seconds(3700)).contains("1h"));
    }

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("hello", 10), "hello");
        assert_eq!(
            truncate_string("hello world this is long", 10),
            "hello w..."
        );
    }

    #[tokio::test]
    async fn test_acquire_and_release() {
        let (_temp_dir, path) = setup();

        // Acquire
        let acquire_args = AcquireArgs {
            scope: "test-scope".to_string(),
            worker_id: "test-worker".to_string(),
            purpose: "Testing".to_string(),
            duration: Some(60),
            locks_dir: Some(path.clone()),
        };
        run_acquire(acquire_args).await.unwrap();

        // Verify locked
        let status_args = StatusArgs {
            scope: "test-scope".to_string(),
            locks_dir: Some(path.clone()),
        };
        run_status(status_args).await.unwrap();

        // Release
        let release_args = ReleaseArgs {
            scope: "test-scope".to_string(),
            worker_id: "test-worker".to_string(),
            locks_dir: Some(path.clone()),
        };
        run_release(release_args).await.unwrap();
    }

    #[tokio::test]
    async fn test_list_empty() {
        let (_temp_dir, path) = setup();

        let args = ListArgs {
            active: false,
            expired: false,
            locks_dir: Some(path),
        };
        run_list(args).await.unwrap();
    }

    #[tokio::test]
    async fn test_mine() {
        let (_temp_dir, path) = setup();

        // Create a lock first
        let manager = LockManager::new(&path).unwrap();
        manager
            .acquire("test-scope", "my-worker", "Testing")
            .unwrap();

        let args = MineArgs {
            worker_id: "my-worker".to_string(),
            locks_dir: Some(path),
        };
        run_mine(args).await.unwrap();
    }

    #[tokio::test]
    async fn test_cleanup() {
        let (_temp_dir, path) = setup();

        let args = CleanupArgs {
            locks_dir: Some(path),
        };
        run_cleanup(args).await.unwrap();
    }
}
