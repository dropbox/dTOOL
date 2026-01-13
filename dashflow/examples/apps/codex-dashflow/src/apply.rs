//! Git patch/apply workflow for Codex DashFlow.
//!
//! Provides functionality to generate unified diffs from file changes and
//! apply them via `git apply`. Supports dry-run mode for previewing changes.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tracing::info;

/// Result of a git apply operation.
#[derive(Debug)]
pub struct ApplyResult {
    /// Whether the operation succeeded.
    pub success: bool,
    /// Output message from the operation.
    pub message: String,
    /// Files that were modified (if apply succeeded).
    pub modified_files: Vec<PathBuf>,
}

/// Generate a unified diff between two strings (original and modified content).
///
/// Uses a simple line-based diff algorithm suitable for most code changes.
pub fn generate_unified_diff(
    file_path: &str,
    original: &str,
    modified: &str,
) -> String {
    let mut diff = String::new();

    // Unified diff header
    diff.push_str(&format!("--- a/{}\n", file_path));
    diff.push_str(&format!("+++ b/{}\n", file_path));

    let original_lines: Vec<&str> = original.lines().collect();
    let modified_lines: Vec<&str> = modified.lines().collect();

    // Simple diff algorithm - find changed regions
    let hunks = compute_hunks(&original_lines, &modified_lines);

    for hunk in hunks {
        diff.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            hunk.orig_start + 1,
            hunk.orig_count,
            hunk.mod_start + 1,
            hunk.mod_count
        ));

        // Output context and changes
        for line in &hunk.lines {
            match line {
                DiffLine::Context(s) => diff.push_str(&format!(" {}\n", s)),
                DiffLine::Removed(s) => diff.push_str(&format!("-{}\n", s)),
                DiffLine::Added(s) => diff.push_str(&format!("+{}\n", s)),
            }
        }
    }

    diff
}

#[derive(Debug)]
enum DiffLine<'a> {
    Context(&'a str),
    Removed(&'a str),
    Added(&'a str),
}

#[derive(Debug)]
struct DiffHunk<'a> {
    orig_start: usize,
    orig_count: usize,
    mod_start: usize,
    mod_count: usize,
    lines: Vec<DiffLine<'a>>,
}

fn compute_hunks<'a>(original: &[&'a str], modified: &[&'a str]) -> Vec<DiffHunk<'a>> {
    // Use a simple LCS-based diff algorithm
    let lcs = longest_common_subsequence(original, modified);

    let mut hunks = Vec::new();
    let mut current_hunk: Option<DiffHunk<'a>> = None;

    let mut i = 0; // original index
    let mut j = 0; // modified index
    let mut lcs_idx = 0;

    // Context lines to include before/after changes
    const CONTEXT_LINES: usize = 3;

    while i < original.len() || j < modified.len() {
        // Check if current lines match the LCS
        let at_common = lcs_idx < lcs.len()
            && i < original.len()
            && j < modified.len()
            && original[i] == lcs[lcs_idx]
            && modified[j] == lcs[lcs_idx];

        if at_common {
            // Common line - add as context if in a hunk
            if let Some(ref mut hunk) = current_hunk {
                hunk.lines.push(DiffLine::Context(original[i]));
                hunk.orig_count += 1;
                hunk.mod_count += 1;
            }
            i += 1;
            j += 1;
            lcs_idx += 1;
        } else {
            // Difference found - start or extend hunk
            let hunk = current_hunk.get_or_insert_with(|| {
                // Start new hunk with context
                let ctx_start_orig = i.saturating_sub(CONTEXT_LINES);
                let ctx_start_mod = j.saturating_sub(CONTEXT_LINES);

                let mut new_hunk = DiffHunk {
                    orig_start: ctx_start_orig,
                    orig_count: 0,
                    mod_start: ctx_start_mod,
                    mod_count: 0,
                    lines: Vec::new(),
                };

                // Add leading context
                for k in ctx_start_orig..i {
                    if k < original.len() {
                        new_hunk.lines.push(DiffLine::Context(original[k]));
                        new_hunk.orig_count += 1;
                        new_hunk.mod_count += 1;
                    }
                }

                new_hunk
            });

            // Determine what changed
            let orig_in_lcs = lcs_idx < lcs.len()
                && i < original.len()
                && original[i] == lcs[lcs_idx];
            let mod_in_lcs = lcs_idx < lcs.len()
                && j < modified.len()
                && modified[j] == lcs[lcs_idx];

            if !orig_in_lcs && i < original.len() {
                // Line removed from original
                hunk.lines.push(DiffLine::Removed(original[i]));
                hunk.orig_count += 1;
                i += 1;
            } else if !mod_in_lcs && j < modified.len() {
                // Line added to modified
                hunk.lines.push(DiffLine::Added(modified[j]));
                hunk.mod_count += 1;
                j += 1;
            }
        }

        // Check if we should close the current hunk
        // (no more changes nearby)
        let should_close = current_hunk.as_ref().is_some_and(|hunk| {
            let last_change_idx = hunk.lines.iter().rposition(|l| {
                matches!(l, DiffLine::Added(_) | DiffLine::Removed(_))
            });

            if let Some(idx) = last_change_idx {
                let context_after = hunk.lines.len() - idx - 1;
                context_after >= CONTEXT_LINES && at_common
            } else {
                false
            }
        });

        if should_close {
            if let Some(hunk) = current_hunk.take() {
                // Enough trailing context, close the hunk
                hunks.push(hunk);
            }
        }
    }

    // Don't forget the last hunk
    if let Some(hunk) = current_hunk {
        hunks.push(hunk);
    }

    hunks
}

fn longest_common_subsequence<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<&'a str> {
    let m = a.len();
    let n = b.len();

    if m == 0 || n == 0 {
        return Vec::new();
    }

    // Build LCS length table
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for (i, &ai) in a.iter().enumerate() {
        for (j, &bj) in b.iter().enumerate() {
            if ai == bj {
                dp[i + 1][j + 1] = dp[i][j] + 1;
            } else {
                dp[i + 1][j + 1] = dp[i + 1][j].max(dp[i][j + 1]);
            }
        }
    }

    // Backtrack to find LCS
    let mut lcs = Vec::new();
    let mut i = m;
    let mut j = n;
    while i > 0 && j > 0 {
        if a[i - 1] == b[j - 1] {
            lcs.push(a[i - 1]);
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] > dp[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }

    lcs.reverse();
    lcs
}

/// Apply a patch using `git apply`.
///
/// # Arguments
/// * `working_dir` - The git repository directory
/// * `patch_content` - The unified diff patch content
/// * `dry_run` - If true, only check if the patch applies cleanly
pub async fn git_apply(
    working_dir: &Path,
    patch_content: &str,
    dry_run: bool,
) -> Result<ApplyResult> {
    info!(
        working_dir = %working_dir.display(),
        dry_run,
        patch_len = patch_content.len(),
        "Applying git patch"
    );

    // Check if we're in a git repository
    let git_check = Command::new("git")
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .current_dir(working_dir)
        .output()
        .await
        .context("Failed to run git rev-parse")?;

    if !git_check.status.success() {
        bail!(
            "Not a git repository: {}. The apply command requires a git working tree.",
            working_dir.display()
        );
    }

    // Build git apply command
    let mut cmd = Command::new("git");
    cmd.arg("apply");

    if dry_run {
        cmd.arg("--check");
    }

    // Apply with some tolerance for whitespace
    cmd.arg("--whitespace=fix");

    // Read patch from stdin
    cmd.arg("-");

    cmd.current_dir(working_dir);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().context("Failed to spawn git apply")?;

    // Write patch to stdin
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        stdin
            .write_all(patch_content.as_bytes())
            .await
            .context("Failed to write patch to git apply stdin")?;
    }

    let output = child.wait_with_output().await.context("Failed to wait for git apply")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        // Parse modified files from the patch
        let modified_files: Vec<PathBuf> = patch_content
            .lines()
            .filter(|line| line.starts_with("+++ b/"))
            .map(|line| PathBuf::from(&line[6..]))
            .collect();

        let message = if dry_run {
            format!("Patch applies cleanly. {} file(s) would be modified.", modified_files.len())
        } else {
            format!("Successfully applied patch. {} file(s) modified.", modified_files.len())
        };

        Ok(ApplyResult {
            success: true,
            message,
            modified_files,
        })
    } else {
        let error_msg = if stderr.is_empty() {
            stdout.to_string()
        } else {
            stderr.to_string()
        };

        Ok(ApplyResult {
            success: false,
            message: format!("Patch failed to apply: {}", error_msg.trim()),
            modified_files: Vec::new(),
        })
    }
}

/// Generate a patch from git working tree changes.
///
/// Returns the unified diff of unstaged changes.
pub async fn git_diff(working_dir: &Path) -> Result<String> {
    let output = Command::new("git")
        .arg("diff")
        .current_dir(working_dir)
        .output()
        .await
        .context("Failed to run git diff")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git diff failed: {}", stderr);
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Generate a patch from staged changes.
pub async fn git_diff_staged(working_dir: &Path) -> Result<String> {
    let output = Command::new("git")
        .arg("diff")
        .arg("--cached")
        .current_dir(working_dir)
        .output()
        .await
        .context("Failed to run git diff --cached")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git diff --cached failed: {}", stderr);
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_unified_diff_simple() {
        let original = "line1\nline2\nline3\n";
        let modified = "line1\nmodified\nline3\n";

        let diff = generate_unified_diff("test.txt", original, modified);

        assert!(diff.contains("--- a/test.txt"));
        assert!(diff.contains("+++ b/test.txt"));
        assert!(diff.contains("-line2"));
        assert!(diff.contains("+modified"));
    }

    #[test]
    fn test_generate_unified_diff_addition() {
        let original = "line1\nline2\n";
        let modified = "line1\nnew_line\nline2\n";

        let diff = generate_unified_diff("test.txt", original, modified);

        assert!(diff.contains("+new_line"));
    }

    #[test]
    fn test_generate_unified_diff_deletion() {
        let original = "line1\nto_delete\nline2\n";
        let modified = "line1\nline2\n";

        let diff = generate_unified_diff("test.txt", original, modified);

        assert!(diff.contains("-to_delete"));
    }

    #[test]
    fn test_lcs_empty() {
        let a: Vec<&str> = vec![];
        let b: Vec<&str> = vec!["a", "b"];
        assert!(longest_common_subsequence(&a, &b).is_empty());
    }

    #[test]
    fn test_lcs_identical() {
        let a = vec!["a", "b", "c"];
        let b = vec!["a", "b", "c"];
        assert_eq!(longest_common_subsequence(&a, &b), vec!["a", "b", "c"]);
    }

    #[test]
    fn test_lcs_partial_match() {
        let a = vec!["a", "b", "c", "d"];
        let b = vec!["a", "x", "c", "y"];
        let lcs = longest_common_subsequence(&a, &b);
        assert!(lcs.contains(&"a"));
        assert!(lcs.contains(&"c"));
    }
}
