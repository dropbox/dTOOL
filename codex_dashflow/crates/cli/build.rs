//! Build script for codex-dashflow-cli
//!
//! Captures build-time information for the --version flag:
//! - Git commit hash
//! - Git commit date
//! - Build timestamp
//! - Target platform

use std::process::Command;

fn main() {
    // Tell Cargo to rerun if git HEAD changes
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/heads/");

    // Get git commit hash
    let git_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Get git commit date
    let git_date = Command::new("git")
        .args(["log", "-1", "--format=%ci"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| {
            // Extract just the date part (YYYY-MM-DD)
            s.split_whitespace().next().unwrap_or("unknown").to_string()
        })
        .unwrap_or_else(|| "unknown".to_string());

    // Check if the working tree is dirty
    let git_dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);

    let git_hash_with_dirty = if git_dirty {
        format!("{}-dirty", git_hash)
    } else {
        git_hash
    };

    // Get build timestamp
    let build_timestamp = chrono::Utc::now()
        .format("%Y-%m-%d %H:%M:%S UTC")
        .to_string();

    // Get target platform
    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());

    // Output environment variables for use in the code
    println!("cargo:rustc-env=GIT_HASH={}", git_hash_with_dirty);
    println!("cargo:rustc-env=GIT_DATE={}", git_date);
    println!("cargo:rustc-env=BUILD_TIMESTAMP={}", build_timestamp);
    println!("cargo:rustc-env=BUILD_TARGET={}", target);
}
