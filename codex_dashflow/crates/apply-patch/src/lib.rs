//! Apply patch - pure Rust implementation for applying patches to files
//!
//! This module provides functionality to parse and apply patches in the
//! "apply_patch" format used by coding assistants.

mod parser;
mod seek_sequence;

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use parser::ParseError::*;
pub use parser::{parse_patch, Hunk, ParseError, UpdateFileChunk};
use thiserror::Error;

/// Detailed instructions for LLMs on how to use the `apply_patch` tool.
pub const APPLY_PATCH_TOOL_INSTRUCTIONS: &str = include_str!("../apply_patch_tool_instructions.md");

/// Both the raw PATCH argument to `apply_patch` as well as the PATCH argument
/// parsed into hunks.
#[derive(Debug, PartialEq)]
pub struct ApplyPatchArgs {
    pub patch: String,
    pub hunks: Vec<Hunk>,
    pub workdir: Option<String>,
}

#[derive(Debug, Error, PartialEq)]
pub enum ApplyPatchError {
    #[error(transparent)]
    ParseError(#[from] ParseError),
    #[error(transparent)]
    IoError(#[from] IoError),
    #[error("{0}")]
    ComputeReplacements(String),
    #[error("patch detected without explicit call to apply_patch. Rerun as [\"apply_patch\", \"<patch>\"]")]
    ImplicitInvocation,
}

impl From<std::io::Error> for ApplyPatchError {
    fn from(err: std::io::Error) -> Self {
        ApplyPatchError::IoError(IoError {
            context: "I/O error".to_string(),
            source: err,
        })
    }
}

#[derive(Debug, Error)]
#[error("{context}: {source}")]
pub struct IoError {
    context: String,
    #[source]
    source: std::io::Error,
}

impl PartialEq for IoError {
    fn eq(&self, other: &Self) -> bool {
        self.context == other.context && self.source.to_string() == other.source.to_string()
    }
}

/// Tracks file paths affected by applying a patch.
pub struct AffectedPaths {
    pub added: Vec<PathBuf>,
    pub modified: Vec<PathBuf>,
    pub deleted: Vec<PathBuf>,
}

/// Applies the patch and prints the result to stdout/stderr.
pub fn apply_patch(
    patch: &str,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Result<(), ApplyPatchError> {
    let hunks = match parse_patch(patch) {
        Ok(source) => source.hunks,
        Err(e) => {
            match &e {
                InvalidPatchError(message) => {
                    writeln!(stderr, "Invalid patch: {message}").map_err(ApplyPatchError::from)?;
                }
                InvalidHunkError {
                    message,
                    line_number,
                } => {
                    writeln!(
                        stderr,
                        "Invalid patch hunk on line {line_number}: {message}"
                    )
                    .map_err(ApplyPatchError::from)?;
                }
            }
            return Err(ApplyPatchError::ParseError(e));
        }
    };

    apply_hunks(&hunks, stdout, stderr)?;
    Ok(())
}

/// Applies hunks and writes progress to stdout/stderr
pub fn apply_hunks(
    hunks: &[Hunk],
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Result<(), ApplyPatchError> {
    match apply_hunks_to_files(hunks) {
        Ok(affected) => {
            print_summary(&affected, stdout).map_err(ApplyPatchError::from)?;
            Ok(())
        }
        Err(err) => {
            let msg = err.to_string();
            writeln!(stderr, "{msg}").map_err(ApplyPatchError::from)?;
            if let Some(io) = err.downcast_ref::<std::io::Error>() {
                Err(ApplyPatchError::IoError(IoError {
                    context: msg,
                    source: std::io::Error::new(io.kind(), io.to_string()),
                }))
            } else {
                Err(ApplyPatchError::IoError(IoError {
                    context: msg,
                    source: std::io::Error::other(err),
                }))
            }
        }
    }
}

/// Apply the hunks to the filesystem
fn apply_hunks_to_files(hunks: &[Hunk]) -> anyhow::Result<AffectedPaths> {
    if hunks.is_empty() {
        anyhow::bail!("No files were modified.");
    }

    let mut added: Vec<PathBuf> = Vec::new();
    let mut modified: Vec<PathBuf> = Vec::new();
    let mut deleted: Vec<PathBuf> = Vec::new();

    for hunk in hunks {
        match hunk {
            Hunk::AddFile { path, contents } => {
                if let Some(parent) = path.parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent).with_context(|| {
                            format!("Failed to create parent directories for {}", path.display())
                        })?;
                    }
                }
                std::fs::write(path, contents)
                    .with_context(|| format!("Failed to write file {}", path.display()))?;
                added.push(path.clone());
            }
            Hunk::DeleteFile { path } => {
                std::fs::remove_file(path)
                    .with_context(|| format!("Failed to delete file {}", path.display()))?;
                deleted.push(path.clone());
            }
            Hunk::UpdateFile {
                path,
                move_path,
                chunks,
            } => {
                let AppliedPatch { new_contents, .. } =
                    derive_new_contents_from_chunks(path, chunks)?;
                if let Some(dest) = move_path {
                    if let Some(parent) = dest.parent() {
                        if !parent.as_os_str().is_empty() {
                            std::fs::create_dir_all(parent).with_context(|| {
                                format!(
                                    "Failed to create parent directories for {}",
                                    dest.display()
                                )
                            })?;
                        }
                    }
                    std::fs::write(dest, new_contents)
                        .with_context(|| format!("Failed to write file {}", dest.display()))?;
                    std::fs::remove_file(path)
                        .with_context(|| format!("Failed to remove original {}", path.display()))?;
                    modified.push(dest.clone());
                } else {
                    std::fs::write(path, new_contents)
                        .with_context(|| format!("Failed to write file {}", path.display()))?;
                    modified.push(path.clone());
                }
            }
        }
    }

    Ok(AffectedPaths {
        added,
        modified,
        deleted,
    })
}

struct AppliedPatch {
    #[allow(dead_code)]
    original_contents: String,
    new_contents: String,
}

fn derive_new_contents_from_chunks(
    path: &Path,
    chunks: &[UpdateFileChunk],
) -> std::result::Result<AppliedPatch, ApplyPatchError> {
    let original_contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) => {
            return Err(ApplyPatchError::IoError(IoError {
                context: format!("Failed to read file to update {}", path.display()),
                source: err,
            }));
        }
    };

    let mut original_lines: Vec<String> = original_contents.split('\n').map(String::from).collect();

    // Drop the trailing empty element that results from the final newline
    if original_lines.last().is_some_and(String::is_empty) {
        original_lines.pop();
    }

    let replacements = compute_replacements(&original_lines, path, chunks)?;
    let new_lines = apply_replacements(original_lines, &replacements);
    let mut new_lines = new_lines;
    if !new_lines.last().is_some_and(String::is_empty) {
        new_lines.push(String::new());
    }
    let new_contents = new_lines.join("\n");
    Ok(AppliedPatch {
        original_contents,
        new_contents,
    })
}

fn compute_replacements(
    original_lines: &[String],
    path: &Path,
    chunks: &[UpdateFileChunk],
) -> std::result::Result<Vec<(usize, usize, Vec<String>)>, ApplyPatchError> {
    let mut replacements: Vec<(usize, usize, Vec<String>)> = Vec::new();
    let mut line_index: usize = 0;

    for chunk in chunks {
        // Find context if specified
        if let Some(ctx_line) = &chunk.change_context {
            if let Some(idx) = seek_sequence::seek_sequence(
                original_lines,
                std::slice::from_ref(ctx_line),
                line_index,
                false,
            ) {
                line_index = idx + 1;
            } else {
                return Err(ApplyPatchError::ComputeReplacements(format!(
                    "Failed to find context '{}' in {}",
                    ctx_line,
                    path.display()
                )));
            }
        }

        if chunk.old_lines.is_empty() {
            // Pure addition
            let insertion_idx = if original_lines.last().is_some_and(String::is_empty) {
                original_lines.len() - 1
            } else {
                original_lines.len()
            };
            replacements.push((insertion_idx, 0, chunk.new_lines.clone()));
            continue;
        }

        // Try to match old_lines
        let mut pattern: &[String] = &chunk.old_lines;
        let mut found =
            seek_sequence::seek_sequence(original_lines, pattern, line_index, chunk.is_end_of_file);
        let mut new_slice: &[String] = &chunk.new_lines;

        if found.is_none() && pattern.last().is_some_and(String::is_empty) {
            // Retry without trailing empty line
            pattern = &pattern[..pattern.len() - 1];
            if new_slice.last().is_some_and(String::is_empty) {
                new_slice = &new_slice[..new_slice.len() - 1];
            }
            found = seek_sequence::seek_sequence(
                original_lines,
                pattern,
                line_index,
                chunk.is_end_of_file,
            );
        }

        if let Some(start_idx) = found {
            replacements.push((start_idx, pattern.len(), new_slice.to_vec()));
            line_index = start_idx + pattern.len();
        } else {
            return Err(ApplyPatchError::ComputeReplacements(format!(
                "Failed to find expected lines in {}:\n{}",
                path.display(),
                chunk.old_lines.join("\n"),
            )));
        }
    }

    replacements.sort_by(|(lhs_idx, _, _), (rhs_idx, _, _)| lhs_idx.cmp(rhs_idx));
    Ok(replacements)
}

fn apply_replacements(
    mut lines: Vec<String>,
    replacements: &[(usize, usize, Vec<String>)],
) -> Vec<String> {
    // Apply in reverse order
    for (start_idx, old_len, new_segment) in replacements.iter().rev() {
        let start_idx = *start_idx;
        let old_len = *old_len;

        // Remove old lines
        for _ in 0..old_len {
            if start_idx < lines.len() {
                lines.remove(start_idx);
            }
        }

        // Insert new lines
        for (offset, new_line) in new_segment.iter().enumerate() {
            lines.insert(start_idx + offset, new_line.clone());
        }
    }

    lines
}

/// Print the summary of changes in git-style format.
pub fn print_summary(affected: &AffectedPaths, out: &mut impl Write) -> std::io::Result<()> {
    writeln!(out, "Success. Updated the following files:")?;
    for path in &affected.added {
        writeln!(out, "A {}", path.display())?;
    }
    for path in &affected.modified {
        writeln!(out, "M {}", path.display())?;
    }
    for path in &affected.deleted {
        writeln!(out, "D {}", path.display())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    /// Helper to construct a patch with the given body.
    fn wrap_patch(body: &str) -> String {
        format!("*** Begin Patch\n{body}\n*** End Patch")
    }

    #[test]
    fn test_add_file_hunk_creates_file_with_contents() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("add.txt");
        let patch = wrap_patch(&format!(
            r#"*** Add File: {}
+ab
+cd"#,
            path.display()
        ));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        apply_patch(&patch, &mut stdout, &mut stderr).unwrap();
        let stdout_str = String::from_utf8(stdout).unwrap();
        let expected_out = format!(
            "Success. Updated the following files:\nA {}\n",
            path.display()
        );
        assert_eq!(stdout_str, expected_out);
        let contents = fs::read_to_string(path).unwrap();
        assert_eq!(contents, "ab\ncd\n");
    }

    #[test]
    fn test_delete_file_hunk_removes_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("del.txt");
        fs::write(&path, "x").unwrap();
        let patch = wrap_patch(&format!("*** Delete File: {}", path.display()));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        apply_patch(&patch, &mut stdout, &mut stderr).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn test_update_file_hunk_modifies_content() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("update.txt");
        fs::write(&path, "foo\nbar\n").unwrap();
        let patch = wrap_patch(&format!(
            r#"*** Update File: {}
@@
 foo
-bar
+baz"#,
            path.display()
        ));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        apply_patch(&patch, &mut stdout, &mut stderr).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "foo\nbaz\n");
    }

    #[test]
    fn test_update_file_hunk_can_move_file() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.txt");
        let dest = dir.path().join("dst.txt");
        fs::write(&src, "line\n").unwrap();
        let patch = wrap_patch(&format!(
            r#"*** Update File: {}
*** Move to: {}
@@
-line
+line2"#,
            src.display(),
            dest.display()
        ));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        apply_patch(&patch, &mut stdout, &mut stderr).unwrap();
        assert!(!src.exists());
        let contents = fs::read_to_string(&dest).unwrap();
        assert_eq!(contents, "line2\n");
    }

    #[test]
    fn test_multiple_update_chunks_apply_to_single_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("multi.txt");
        fs::write(&path, "foo\nbar\nbaz\nqux\n").unwrap();
        let patch = wrap_patch(&format!(
            r#"*** Update File: {}
@@
 foo
-bar
+BAR
@@
 baz
-qux
+QUX"#,
            path.display()
        ));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        apply_patch(&patch, &mut stdout, &mut stderr).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "foo\nBAR\nbaz\nQUX\n");
    }

    #[test]
    fn test_apply_patch_args_struct() {
        let args = ApplyPatchArgs {
            patch: "test patch".to_string(),
            hunks: vec![],
            workdir: Some("/tmp".to_string()),
        };
        assert_eq!(args.patch, "test patch");
        assert!(args.hunks.is_empty());
        assert_eq!(args.workdir, Some("/tmp".to_string()));
    }

    #[test]
    fn test_apply_patch_args_no_workdir() {
        let args = ApplyPatchArgs {
            patch: "patch content".to_string(),
            hunks: vec![],
            workdir: None,
        };
        assert!(args.workdir.is_none());
    }

    #[test]
    fn test_affected_paths_struct() {
        let affected = AffectedPaths {
            added: vec![PathBuf::from("new.txt")],
            modified: vec![PathBuf::from("changed.txt")],
            deleted: vec![PathBuf::from("removed.txt")],
        };
        assert_eq!(affected.added.len(), 1);
        assert_eq!(affected.modified.len(), 1);
        assert_eq!(affected.deleted.len(), 1);
    }

    #[test]
    fn test_affected_paths_empty() {
        let affected = AffectedPaths {
            added: vec![],
            modified: vec![],
            deleted: vec![],
        };
        assert!(affected.added.is_empty());
        assert!(affected.modified.is_empty());
        assert!(affected.deleted.is_empty());
    }

    #[test]
    fn test_print_summary_added_only() {
        let affected = AffectedPaths {
            added: vec![PathBuf::from("new1.txt"), PathBuf::from("new2.txt")],
            modified: vec![],
            deleted: vec![],
        };
        let mut out = Vec::new();
        print_summary(&affected, &mut out).unwrap();
        let output = String::from_utf8(out).unwrap();
        assert!(output.contains("A new1.txt"));
        assert!(output.contains("A new2.txt"));
        assert!(output.contains("Success"));
    }

    #[test]
    fn test_print_summary_all_types() {
        let affected = AffectedPaths {
            added: vec![PathBuf::from("added.txt")],
            modified: vec![PathBuf::from("modified.txt")],
            deleted: vec![PathBuf::from("deleted.txt")],
        };
        let mut out = Vec::new();
        print_summary(&affected, &mut out).unwrap();
        let output = String::from_utf8(out).unwrap();
        assert!(output.contains("A added.txt"));
        assert!(output.contains("M modified.txt"));
        assert!(output.contains("D deleted.txt"));
    }

    #[test]
    fn test_apply_patch_error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let apply_err: ApplyPatchError = io_err.into();
        match apply_err {
            ApplyPatchError::IoError(e) => {
                assert!(e.to_string().contains("I/O error"));
            }
            _ => panic!("Expected IoError variant"),
        }
    }

    #[test]
    fn test_apply_patch_error_compute_replacements() {
        let err = ApplyPatchError::ComputeReplacements("context not found".to_string());
        assert!(err.to_string().contains("context not found"));
    }

    #[test]
    fn test_apply_patch_error_implicit_invocation() {
        let err = ApplyPatchError::ImplicitInvocation;
        assert!(err
            .to_string()
            .contains("patch detected without explicit call"));
    }

    #[test]
    fn test_io_error_equality() {
        let err1 = IoError {
            context: "ctx".to_string(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
        };
        let err2 = IoError {
            context: "ctx".to_string(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
        };
        assert_eq!(err1, err2);
    }

    #[test]
    fn test_io_error_display() {
        let err = IoError {
            context: "failed to read file".to_string(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "missing"),
        };
        let display = format!("{}", err);
        assert!(display.contains("failed to read file"));
    }

    #[test]
    fn test_apply_patch_invalid_patch_error() {
        let patch = "not a valid patch format";
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let result = apply_patch(patch, &mut stdout, &mut stderr);
        assert!(result.is_err());
        let stderr_str = String::from_utf8(stderr).unwrap();
        assert!(stderr_str.contains("Invalid patch"));
    }

    #[test]
    fn test_apply_hunks_empty_returns_error() {
        let hunks: Vec<Hunk> = vec![];
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let result = apply_hunks(&hunks, &mut stdout, &mut stderr);
        assert!(result.is_err());
        let stderr_str = String::from_utf8(stderr).unwrap();
        assert!(stderr_str.contains("No files were modified"));
    }

    #[test]
    fn test_add_file_creates_parent_directories() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("deep").join("file.txt");
        let patch = wrap_patch(&format!(
            r#"*** Add File: {}
+content"#,
            path.display()
        ));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        apply_patch(&patch, &mut stdout, &mut stderr).unwrap();
        assert!(path.exists());
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "content\n");
    }

    #[test]
    fn test_add_file_empty_content() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("empty.txt");
        let patch = wrap_patch(&format!(r#"*** Add File: {}"#, path.display()));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        apply_patch(&patch, &mut stdout, &mut stderr).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_delete_nonexistent_file_fails() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("does_not_exist.txt");
        let patch = wrap_patch(&format!("*** Delete File: {}", path.display()));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let result = apply_patch(&patch, &mut stdout, &mut stderr);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_file_context_line() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("ctx.txt");
        fs::write(&path, "header\nfoo\nbar\nfooter\n").unwrap();
        let patch = wrap_patch(&format!(
            r#"*** Update File: {}
@@ foo
-bar
+BAR"#,
            path.display()
        ));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        apply_patch(&patch, &mut stdout, &mut stderr).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "header\nfoo\nBAR\nfooter\n");
    }

    #[test]
    fn test_update_file_missing_context_fails() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("noctx.txt");
        fs::write(&path, "line1\nline2\n").unwrap();
        let patch = wrap_patch(&format!(
            r#"*** Update File: {}
@@ nonexistent_context_line
-line2
+CHANGED"#,
            path.display()
        ));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let result = apply_patch(&patch, &mut stdout, &mut stderr);
        assert!(result.is_err());
    }

    #[test]
    fn test_move_file_creates_destination_directory() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.txt");
        let dest = dir.path().join("new_dir").join("dest.txt");
        fs::write(&src, "content\n").unwrap();
        let patch = wrap_patch(&format!(
            r#"*** Update File: {}
*** Move to: {}
@@
-content
+modified content"#,
            src.display(),
            dest.display()
        ));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        apply_patch(&patch, &mut stdout, &mut stderr).unwrap();
        assert!(!src.exists());
        assert!(dest.exists());
        let contents = fs::read_to_string(&dest).unwrap();
        assert_eq!(contents, "modified content\n");
    }

    #[test]
    fn test_update_nonexistent_file_fails() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("missing.txt");
        let patch = wrap_patch(&format!(
            r#"*** Update File: {}
@@
-old
+new"#,
            path.display()
        ));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let result = apply_patch(&patch, &mut stdout, &mut stderr);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_pattern_not_found_fails() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nopat.txt");
        fs::write(&path, "actual\ncontent\n").unwrap();
        let patch = wrap_patch(&format!(
            r#"*** Update File: {}
@@
-wrong pattern
+replacement"#,
            path.display()
        ));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let result = apply_patch(&patch, &mut stdout, &mut stderr);
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_patch_tool_instructions_not_empty() {
        // Verify the const has expected content (is_empty() check is compile-time guaranteed)
        assert!(APPLY_PATCH_TOOL_INSTRUCTIONS.len() > 100);
        assert!(APPLY_PATCH_TOOL_INSTRUCTIONS.contains("apply_patch"));
    }

    #[test]
    fn test_multiple_files_in_single_patch() {
        let dir = tempdir().unwrap();
        let path1 = dir.path().join("file1.txt");
        let path2 = dir.path().join("file2.txt");
        let patch = wrap_patch(&format!(
            r#"*** Add File: {}
+content1
*** Add File: {}
+content2"#,
            path1.display(),
            path2.display()
        ));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        apply_patch(&patch, &mut stdout, &mut stderr).unwrap();
        assert!(path1.exists());
        assert!(path2.exists());
    }

    #[test]
    fn test_add_file_with_multiple_lines() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("multiline.txt");
        let patch = wrap_patch(&format!(
            r#"*** Add File: {}
+line1
+line2
+line3"#,
            path.display()
        ));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        apply_patch(&patch, &mut stdout, &mut stderr).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "line1\nline2\nline3\n");
    }

    #[test]
    fn test_update_file_pure_addition() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("append.txt");
        fs::write(&path, "existing\n").unwrap();
        let patch = wrap_patch(&format!(
            r#"*** Update File: {}
@@
+appended"#,
            path.display()
        ));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        apply_patch(&patch, &mut stdout, &mut stderr).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("appended"));
    }
}
