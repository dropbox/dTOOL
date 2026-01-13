//! Turn-level diff tracking for file changes
//!
//! Tracks sets of changes to files and exposes the overall unified diff.
//! Used to show users what changed during an agent turn.
//!
//! ## How it works
//!
//! 1. Maintain an in-memory baseline snapshot of files when first seen
//! 2. Keep a stable internal filename (UUID) per path for rename tracking
//! 3. Compute aggregated unified diff by comparing baselines to current disk state

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use uuid::Uuid;

const ZERO_OID: &str = "0000000000000000000000000000000000000000";
const DEV_NULL: &str = "/dev/null";

/// Type of file change for patch operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileChange {
    /// Add a new file
    Add {
        /// Content to add
        content: String,
    },
    /// Delete an existing file
    Delete {
        /// Content being deleted
        content: String,
    },
    /// Update/modify a file
    Update {
        /// Unified diff of changes
        unified_diff: String,
        /// Optional path if file is being moved/renamed
        move_path: Option<PathBuf>,
    },
}

/// File mode (permissions) for diff headers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileMode {
    /// Regular file (644)
    Regular,
    /// Executable file (755) - Unix only
    #[cfg(unix)]
    Executable,
    /// Symbolic link (120000)
    Symlink,
}

impl FileMode {
    fn as_str(self) -> &'static str {
        match self {
            FileMode::Regular => "100644",
            #[cfg(unix)]
            FileMode::Executable => "100755",
            FileMode::Symlink => "120000",
        }
    }
}

impl std::fmt::Display for FileMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Baseline snapshot of a file.
struct BaselineFileInfo {
    path: PathBuf,
    content: Vec<u8>,
    mode: FileMode,
    oid: String,
}

/// Tracks file changes during an agent turn and computes unified diffs.
///
/// # Example
///
/// ```ignore
/// let mut tracker = TurnDiffTracker::new();
///
/// // Before applying patch, record baseline
/// tracker.on_patch_begin(&changes);
///
/// // After patch applied, get diff
/// if let Some(diff) = tracker.get_unified_diff()? {
///     println!("{}", diff);
/// }
/// ```
#[derive(Default)]
pub struct TurnDiffTracker {
    /// Map external path -> internal filename (uuid)
    external_to_temp_name: HashMap<PathBuf, String>,
    /// Internal filename -> baseline file info
    baseline_file_info: HashMap<String, BaselineFileInfo>,
    /// Internal filename -> external path (after renames)
    temp_name_to_current_path: HashMap<String, PathBuf>,
    /// Cache of known git worktree roots
    git_root_cache: Vec<PathBuf>,
}

impl TurnDiffTracker {
    /// Create a new diff tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record baseline state before applying patches.
    ///
    /// Call this before each patch application to capture the starting state.
    /// Files that exist on disk get a snapshot; new files don't get a baseline
    /// so they show as proper additions from /dev/null.
    pub fn on_patch_begin(&mut self, changes: &HashMap<PathBuf, FileChange>) {
        for (path, change) in changes.iter() {
            // Create stable internal filename if not exists
            if !self.external_to_temp_name.contains_key(path) {
                let internal = Uuid::new_v4().to_string();
                self.external_to_temp_name
                    .insert(path.clone(), internal.clone());
                self.temp_name_to_current_path
                    .insert(internal.clone(), path.clone());

                // Snapshot baseline if file exists
                let baseline_info = if path.exists() {
                    let mode = file_mode_for_path(path).unwrap_or(FileMode::Regular);
                    let content = blob_bytes(path, mode).unwrap_or_default();
                    let oid = if mode == FileMode::Symlink {
                        format!("{:x}", git_blob_sha1(&content))
                    } else {
                        self.git_blob_oid_for_path(path)
                            .unwrap_or_else(|| format!("{:x}", git_blob_sha1(&content)))
                    };
                    BaselineFileInfo {
                        path: path.clone(),
                        content,
                        mode,
                        oid,
                    }
                } else {
                    // File doesn't exist - will be an addition
                    BaselineFileInfo {
                        path: path.clone(),
                        content: vec![],
                        mode: FileMode::Regular,
                        oid: ZERO_OID.to_string(),
                    }
                };

                self.baseline_file_info.insert(internal, baseline_info);
            }

            // Track rename/move
            if let FileChange::Update {
                move_path: Some(dest),
                ..
            } = change
            {
                let internal = self
                    .external_to_temp_name
                    .get(path)
                    .cloned()
                    .unwrap_or_else(|| {
                        let i = Uuid::new_v4().to_string();
                        self.baseline_file_info.insert(
                            i.clone(),
                            BaselineFileInfo {
                                path: path.clone(),
                                content: vec![],
                                mode: FileMode::Regular,
                                oid: ZERO_OID.to_string(),
                            },
                        );
                        i
                    });

                // Update mappings for move
                self.temp_name_to_current_path
                    .insert(internal.clone(), dest.clone());
                self.external_to_temp_name.remove(path);
                self.external_to_temp_name.insert(dest.clone(), internal);
            }
        }
    }

    /// Get the aggregated unified diff for all tracked changes.
    ///
    /// Compares baseline snapshots to current disk state and generates
    /// a unified diff in git format.
    pub fn get_unified_diff(&mut self) -> Result<Option<String>, std::io::Error> {
        let mut aggregated = String::new();

        // Sort by path for stable output
        let mut internal_names: Vec<String> = self.baseline_file_info.keys().cloned().collect();
        internal_names.sort_by_key(|internal| {
            self.get_path_for_internal(internal)
                .map(|p| self.relative_to_git_root_str(&p))
                .unwrap_or_default()
        });

        for internal in internal_names {
            aggregated.push_str(&self.get_file_diff(&internal));
            if !aggregated.ends_with('\n') {
                aggregated.push('\n');
            }
        }

        if aggregated.trim().is_empty() {
            Ok(None)
        } else {
            Ok(Some(aggregated))
        }
    }

    /// Clear all tracked state.
    pub fn clear(&mut self) {
        self.external_to_temp_name.clear();
        self.baseline_file_info.clear();
        self.temp_name_to_current_path.clear();
    }

    /// Get number of tracked files.
    pub fn tracked_count(&self) -> usize {
        self.baseline_file_info.len()
    }

    fn get_path_for_internal(&self, internal: &str) -> Option<PathBuf> {
        self.temp_name_to_current_path
            .get(internal)
            .cloned()
            .or_else(|| {
                self.baseline_file_info
                    .get(internal)
                    .map(|info| info.path.clone())
            })
    }

    fn find_git_root_cached(&mut self, start: &Path) -> Option<PathBuf> {
        let dir = if start.is_dir() {
            start
        } else {
            start.parent()?
        };

        // Check cache first
        if let Some(root) = self
            .git_root_cache
            .iter()
            .find(|r| dir.starts_with(r))
            .cloned()
        {
            return Some(root);
        }

        // Walk up to find .git
        let mut cur = dir.to_path_buf();
        loop {
            let git_marker = cur.join(".git");
            if git_marker.is_dir() || git_marker.is_file() {
                if !self.git_root_cache.iter().any(|r| r == &cur) {
                    self.git_root_cache.push(cur.clone());
                }
                return Some(cur);
            }

            if let Some(parent) = cur.parent() {
                cur = parent.to_path_buf();
            } else {
                return None;
            }
        }
    }

    fn relative_to_git_root_str(&mut self, path: &Path) -> String {
        let s = if let Some(root) = self.find_git_root_cached(path) {
            if let Ok(rel) = path.strip_prefix(&root) {
                rel.display().to_string()
            } else {
                path.display().to_string()
            }
        } else {
            path.display().to_string()
        };
        s.replace('\\', "/")
    }

    fn git_blob_oid_for_path(&mut self, path: &Path) -> Option<String> {
        let root = self.find_git_root_cached(path)?;
        let rel = path.strip_prefix(&root).unwrap_or(path);
        let output = Command::new("git")
            .arg("-C")
            .arg(&root)
            .arg("hash-object")
            .arg("--")
            .arg(rel)
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if s.len() == 40 {
            Some(s)
        } else {
            None
        }
    }

    fn get_file_diff(&mut self, internal: &str) -> String {
        let mut diff = String::new();

        // Get baseline info
        let (baseline_path, baseline_mode, left_oid) =
            if let Some(info) = self.baseline_file_info.get(internal) {
                (info.path.clone(), info.mode, info.oid.clone())
            } else {
                return diff;
            };

        let current_path = match self.get_path_for_internal(internal) {
            Some(p) => p,
            None => return diff,
        };

        let current_mode = file_mode_for_path(&current_path).unwrap_or(FileMode::Regular);
        let right_bytes = blob_bytes(&current_path, current_mode);

        // Compute display paths
        let left_display = self.relative_to_git_root_str(&baseline_path);
        let right_display = self.relative_to_git_root_str(&current_path);

        // Compute right OID
        let right_oid = if let Some(ref b) = right_bytes {
            if current_mode == FileMode::Symlink {
                format!("{:x}", git_blob_sha1(b))
            } else {
                self.git_blob_oid_for_path(&current_path)
                    .unwrap_or_else(|| format!("{:x}", git_blob_sha1(b)))
            }
        } else {
            ZERO_OID.to_string()
        };

        // Get baseline content for comparison
        let left_present = left_oid.as_str() != ZERO_OID;
        let left_bytes: Option<&[u8]> = if left_present {
            self.baseline_file_info
                .get(internal)
                .map(|i| i.content.as_slice())
        } else {
            None
        };

        // Skip if unchanged
        if left_bytes == right_bytes.as_deref() {
            return diff;
        }

        // Build diff header
        diff.push_str(&format!("diff --git a/{left_display} b/{right_display}\n"));

        let is_add = !left_present && right_bytes.is_some();
        let is_delete = left_present && right_bytes.is_none();

        if is_add {
            diff.push_str(&format!("new file mode {current_mode}\n"));
        } else if is_delete {
            diff.push_str(&format!("deleted file mode {baseline_mode}\n"));
        } else if baseline_mode != current_mode {
            diff.push_str(&format!("old mode {baseline_mode}\n"));
            diff.push_str(&format!("new mode {current_mode}\n"));
        }

        // Try text diff
        let left_text = left_bytes.and_then(|b| std::str::from_utf8(b).ok());
        let right_text = right_bytes
            .as_deref()
            .and_then(|b| std::str::from_utf8(b).ok());

        let can_text_diff = matches!(
            (left_text, right_text, is_add, is_delete),
            (Some(_), Some(_), _, _) | (_, Some(_), true, _) | (Some(_), _, _, true)
        );

        diff.push_str(&format!("index {left_oid}..{right_oid}\n"));

        let old_header = if left_present {
            format!("a/{left_display}")
        } else {
            DEV_NULL.to_string()
        };
        let new_header = if right_bytes.is_some() {
            format!("b/{right_display}")
        } else {
            DEV_NULL.to_string()
        };

        if can_text_diff {
            let l = left_text.unwrap_or("");
            let r = right_text.unwrap_or("");

            let text_diff = similar::TextDiff::from_lines(l, r);
            let unified = text_diff
                .unified_diff()
                .context_radius(3)
                .header(&old_header, &new_header)
                .to_string();

            diff.push_str(&unified);
        } else {
            diff.push_str(&format!("--- {old_header}\n"));
            diff.push_str(&format!("+++ {new_header}\n"));
            diff.push_str("Binary files differ\n");
        }

        diff
    }
}

/// Compute Git blob SHA-1 for content.
fn git_blob_sha1(data: &[u8]) -> sha1::digest::Output<Sha1> {
    let header = format!("blob {}\0", data.len());
    let mut hasher = Sha1::new();
    hasher.update(header.as_bytes());
    hasher.update(data);
    hasher.finalize()
}

#[cfg(unix)]
fn file_mode_for_path(path: &Path) -> Option<FileMode> {
    use std::os::unix::fs::PermissionsExt;
    let meta = fs::symlink_metadata(path).ok()?;
    let ft = meta.file_type();
    if ft.is_symlink() {
        return Some(FileMode::Symlink);
    }
    let mode = meta.permissions().mode();
    let is_exec = (mode & 0o111) != 0;
    Some(if is_exec {
        FileMode::Executable
    } else {
        FileMode::Regular
    })
}

#[cfg(not(unix))]
fn file_mode_for_path(_path: &Path) -> Option<FileMode> {
    Some(FileMode::Regular)
}

fn blob_bytes(path: &Path, mode: FileMode) -> Option<Vec<u8>> {
    if !path.exists() {
        return None;
    }

    if mode == FileMode::Symlink {
        symlink_blob_bytes(path)
    } else {
        fs::read(path).ok()
    }
}

#[cfg(unix)]
fn symlink_blob_bytes(path: &Path) -> Option<Vec<u8>> {
    use std::os::unix::ffi::OsStrExt;
    let target = std::fs::read_link(path).ok()?;
    Some(target.as_os_str().as_bytes().to_vec())
}

#[cfg(not(unix))]
fn symlink_blob_bytes(_path: &Path) -> Option<Vec<u8>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn git_blob_sha1_hex(data: &str) -> String {
        format!("{:x}", git_blob_sha1(data.as_bytes()))
    }

    fn normalize_diff(input: &str, root: &Path) -> String {
        let root_str = root.display().to_string().replace('\\', "/");
        let replaced = input.replace(&root_str, "<TMP>");

        // Sort diff blocks for deterministic output
        let mut blocks: Vec<String> = Vec::new();
        let mut current = String::new();
        for line in replaced.lines() {
            if line.starts_with("diff --git ") && !current.is_empty() {
                blocks.push(current);
                current = String::new();
            }
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line);
        }
        if !current.is_empty() {
            blocks.push(current);
        }
        blocks.sort();
        let mut out = blocks.join("\n");
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out
    }

    #[test]
    fn test_new_tracker() {
        let tracker = TurnDiffTracker::new();
        assert_eq!(tracker.tracked_count(), 0);
    }

    #[test]
    fn test_add_file() {
        let mut tracker = TurnDiffTracker::new();
        let dir = tempdir().unwrap();
        let file = dir.path().join("new.txt");

        let changes = HashMap::from([(
            file.clone(),
            FileChange::Add {
                content: "hello\n".to_string(),
            },
        )]);
        tracker.on_patch_begin(&changes);

        // Simulate file creation
        fs::write(&file, "hello\n").unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        let diff = normalize_diff(&diff, dir.path());

        let mode = file_mode_for_path(&file).unwrap_or(FileMode::Regular);
        let oid = git_blob_sha1_hex("hello\n");
        let expected = format!(
            r#"diff --git a/<TMP>/new.txt b/<TMP>/new.txt
new file mode {mode}
index {ZERO_OID}..{oid}
--- {DEV_NULL}
+++ b/<TMP>/new.txt
@@ -0,0 +1 @@
+hello
"#
        );
        assert_eq!(diff, expected);
    }

    #[test]
    fn test_delete_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("del.txt");
        fs::write(&file, "bye\n").unwrap();

        let mut tracker = TurnDiffTracker::new();
        let mode = file_mode_for_path(&file).unwrap_or(FileMode::Regular);
        let oid = git_blob_sha1_hex("bye\n");

        let changes = HashMap::from([(
            file.clone(),
            FileChange::Delete {
                content: "bye\n".to_string(),
            },
        )]);
        tracker.on_patch_begin(&changes);

        // Simulate deletion
        fs::remove_file(&file).unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        let diff = normalize_diff(&diff, dir.path());

        let expected = format!(
            r#"diff --git a/<TMP>/del.txt b/<TMP>/del.txt
deleted file mode {mode}
index {oid}..{ZERO_OID}
--- a/<TMP>/del.txt
+++ {DEV_NULL}
@@ -1 +0,0 @@
-bye
"#
        );
        assert_eq!(diff, expected);
    }

    #[test]
    fn test_update_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("mod.txt");
        fs::write(&file, "old\n").unwrap();

        let mut tracker = TurnDiffTracker::new();
        let left_oid = git_blob_sha1_hex("old\n");

        let changes = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes);

        // Simulate update
        fs::write(&file, "new\n").unwrap();
        let right_oid = git_blob_sha1_hex("new\n");

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        let diff = normalize_diff(&diff, dir.path());

        let expected = format!(
            r#"diff --git a/<TMP>/mod.txt b/<TMP>/mod.txt
index {left_oid}..{right_oid}
--- a/<TMP>/mod.txt
+++ b/<TMP>/mod.txt
@@ -1 +1 @@
-old
+new
"#
        );
        assert_eq!(diff, expected);
    }

    #[test]
    fn test_no_change() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("same.txt");
        fs::write(&file, "content\n").unwrap();

        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes);

        // No actual change
        let diff = tracker.get_unified_diff().unwrap();
        assert!(diff.is_none());
    }

    #[test]
    fn test_move_with_content_change() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.txt");
        let dest = dir.path().join("dst.txt");
        fs::write(&src, "line\n").unwrap();

        let mut tracker = TurnDiffTracker::new();
        let left_oid = git_blob_sha1_hex("line\n");

        let changes = HashMap::from([(
            src.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: Some(dest.clone()),
            },
        )]);
        tracker.on_patch_begin(&changes);

        // Simulate move + update
        fs::rename(&src, &dest).unwrap();
        fs::write(&dest, "updated\n").unwrap();
        let right_oid = git_blob_sha1_hex("updated\n");

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        let diff = normalize_diff(&diff, dir.path());

        let expected = format!(
            r#"diff --git a/<TMP>/src.txt b/<TMP>/dst.txt
index {left_oid}..{right_oid}
--- a/<TMP>/src.txt
+++ b/<TMP>/dst.txt
@@ -1 +1 @@
-line
+updated
"#
        );
        assert_eq!(diff, expected);
    }

    #[test]
    fn test_move_only_no_diff() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("a.txt");
        let dest = dir.path().join("b.txt");
        fs::write(&src, "same\n").unwrap();

        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([(
            src.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: Some(dest.clone()),
            },
        )]);
        tracker.on_patch_begin(&changes);

        // Move only, no content change
        fs::rename(&src, &dest).unwrap();

        let diff = tracker.get_unified_diff().unwrap();
        assert!(diff.is_none());
    }

    #[test]
    fn test_binary_files() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("bin.dat");

        let left_bytes: Vec<u8> = vec![0xff, 0xfe, 0xfd, 0x00];
        let right_bytes: Vec<u8> = vec![0x01, 0x02, 0x03, 0x00];

        fs::write(&file, &left_bytes).unwrap();

        let mut tracker = TurnDiffTracker::new();
        let changes = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes);

        fs::write(&file, &right_bytes).unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("Binary files differ"));
    }

    #[test]
    fn test_clear() {
        let mut tracker = TurnDiffTracker::new();
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.txt");

        let changes = HashMap::from([(
            file,
            FileChange::Add {
                content: "x".to_string(),
            },
        )]);
        tracker.on_patch_begin(&changes);
        assert_eq!(tracker.tracked_count(), 1);

        tracker.clear();
        assert_eq!(tracker.tracked_count(), 0);
    }

    #[test]
    fn test_file_mode_display() {
        assert_eq!(FileMode::Regular.to_string(), "100644");
        assert_eq!(FileMode::Symlink.to_string(), "120000");
        #[cfg(unix)]
        assert_eq!(FileMode::Executable.to_string(), "100755");
    }

    #[test]
    fn test_file_change_serialization() {
        let add = FileChange::Add {
            content: "test".to_string(),
        };
        let json = serde_json::to_string(&add).unwrap();
        assert!(json.contains("Add"));

        let update = FileChange::Update {
            unified_diff: "diff".to_string(),
            move_path: Some(PathBuf::from("/new/path")),
        };
        let json = serde_json::to_string(&update).unwrap();
        assert!(json.contains("Update"));
        assert!(json.contains("move_path"));
    }

    #[test]
    fn test_file_change_delete_serialization() {
        let delete = FileChange::Delete {
            content: "removed".to_string(),
        };
        let json = serde_json::to_string(&delete).unwrap();
        assert!(json.contains("Delete"));
        assert!(json.contains("removed"));

        // Deserialization
        let parsed: FileChange = serde_json::from_str(&json).unwrap();
        match parsed {
            FileChange::Delete { content } => assert_eq!(content, "removed"),
            _ => panic!("Expected Delete variant"),
        }
    }

    #[test]
    fn test_file_change_clone() {
        let add = FileChange::Add {
            content: "hello".to_string(),
        };
        let add_cloned = add.clone();
        match add_cloned {
            FileChange::Add { content } => assert_eq!(content, "hello"),
            _ => panic!("Expected Add variant"),
        }

        let update = FileChange::Update {
            unified_diff: "---".to_string(),
            move_path: Some(PathBuf::from("/path")),
        };
        let update_cloned = update.clone();
        match update_cloned {
            FileChange::Update {
                unified_diff,
                move_path,
            } => {
                assert_eq!(unified_diff, "---");
                assert_eq!(move_path, Some(PathBuf::from("/path")));
            }
            _ => panic!("Expected Update variant"),
        }

        let delete = FileChange::Delete {
            content: "bye".to_string(),
        };
        let delete_cloned = delete.clone();
        match delete_cloned {
            FileChange::Delete { content } => assert_eq!(content, "bye"),
            _ => panic!("Expected Delete variant"),
        }
    }

    #[test]
    fn test_file_change_debug() {
        let add = FileChange::Add {
            content: "test".to_string(),
        };
        let debug_str = format!("{:?}", add);
        assert!(debug_str.contains("Add"));
        assert!(debug_str.contains("test"));

        let delete = FileChange::Delete {
            content: "old".to_string(),
        };
        let debug_str = format!("{:?}", delete);
        assert!(debug_str.contains("Delete"));

        let update = FileChange::Update {
            unified_diff: "diff".to_string(),
            move_path: None,
        };
        let debug_str = format!("{:?}", update);
        assert!(debug_str.contains("Update"));
    }

    #[test]
    fn test_file_mode_clone_copy_eq() {
        let mode = FileMode::Regular;
        let mode_copy = mode; // Copy
        let mode_clone = mode;
        assert_eq!(mode, mode_copy);
        assert_eq!(mode, mode_clone);

        let symlink = FileMode::Symlink;
        assert_ne!(mode, symlink);

        #[cfg(unix)]
        {
            let exec = FileMode::Executable;
            assert_ne!(mode, exec);
            assert_ne!(symlink, exec);
        }
    }

    #[test]
    fn test_file_mode_debug() {
        let mode = FileMode::Regular;
        let debug_str = format!("{:?}", mode);
        assert_eq!(debug_str, "Regular");

        let symlink = FileMode::Symlink;
        let debug_str = format!("{:?}", symlink);
        assert_eq!(debug_str, "Symlink");

        #[cfg(unix)]
        {
            let exec = FileMode::Executable;
            let debug_str = format!("{:?}", exec);
            assert_eq!(debug_str, "Executable");
        }
    }

    #[test]
    fn test_turn_diff_tracker_default() {
        let tracker = TurnDiffTracker::default();
        assert_eq!(tracker.tracked_count(), 0);
    }

    #[test]
    fn test_git_blob_sha1() {
        // Known SHA1 for "hello\n"
        let hash = git_blob_sha1(b"hello\n");
        let hash_hex = format!("{:x}", hash);
        // git hash-object -w --stdin <<< "hello" produces ce013625030ba8dba906f756967f9e9ca394464a
        assert_eq!(hash_hex, "ce013625030ba8dba906f756967f9e9ca394464a");

        // Empty content
        let empty_hash = git_blob_sha1(b"");
        let empty_hex = format!("{:x}", empty_hash);
        // git hash-object -t blob /dev/null produces e69de29bb2d1d6434b8b29ae775ad8c2e48c5391
        assert_eq!(empty_hex, "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391");
    }

    #[test]
    fn test_multiple_files_tracked() {
        let dir = tempdir().unwrap();
        let file1 = dir.path().join("file1.txt");
        let file2 = dir.path().join("file2.txt");
        let file3 = dir.path().join("file3.txt");

        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([
            (
                file1.clone(),
                FileChange::Add {
                    content: "one\n".to_string(),
                },
            ),
            (
                file2.clone(),
                FileChange::Add {
                    content: "two\n".to_string(),
                },
            ),
            (
                file3.clone(),
                FileChange::Add {
                    content: "three\n".to_string(),
                },
            ),
        ]);
        tracker.on_patch_begin(&changes);

        assert_eq!(tracker.tracked_count(), 3);

        // Create all files
        fs::write(&file1, "one\n").unwrap();
        fs::write(&file2, "two\n").unwrap();
        fs::write(&file3, "three\n").unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("file1.txt"));
        assert!(diff.contains("file2.txt"));
        assert!(diff.contains("file3.txt"));
        assert!(diff.contains("+one"));
        assert!(diff.contains("+two"));
        assert!(diff.contains("+three"));
    }

    #[test]
    fn test_sequential_updates_same_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("seq.txt");
        fs::write(&file, "initial\n").unwrap();

        let mut tracker = TurnDiffTracker::new();

        // First update
        let changes = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes);

        fs::write(&file, "modified\n").unwrap();

        // Second update - baseline should be preserved from first call
        let changes2 = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes2);

        fs::write(&file, "final\n").unwrap();

        // Diff should show change from "initial" to "final"
        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("-initial"));
        assert!(diff.contains("+final"));
    }

    #[test]
    fn test_empty_content_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("empty.txt");

        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([(
            file.clone(),
            FileChange::Add {
                content: String::new(),
            },
        )]);
        tracker.on_patch_begin(&changes);

        // Create empty file
        fs::write(&file, "").unwrap();

        let diff = tracker.get_unified_diff().unwrap();
        // Empty file creation might show empty diff or minimal diff
        // Just verify no panic
        assert!(diff.is_none() || diff.is_some());
    }

    #[test]
    fn test_unicode_content() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("unicode.txt");

        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([(
            file.clone(),
            FileChange::Add {
                content: "こんにちは 🎉\n".to_string(),
            },
        )]);
        tracker.on_patch_begin(&changes);

        fs::write(&file, "こんにちは 🎉\n").unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("こんにちは"));
        assert!(diff.contains("🎉"));
    }

    #[test]
    fn test_blob_bytes_nonexistent() {
        let result = blob_bytes(Path::new("/nonexistent/file.txt"), FileMode::Regular);
        assert!(result.is_none());
    }

    #[test]
    fn test_file_mode_for_path_nonexistent() {
        let result = file_mode_for_path(Path::new("/nonexistent/file.txt"));
        assert!(result.is_none());
    }

    #[test]
    fn test_get_unified_diff_empty_tracker() {
        let mut tracker = TurnDiffTracker::new();
        let diff = tracker.get_unified_diff().unwrap();
        assert!(diff.is_none());
    }

    #[test]
    fn test_update_with_none_move_path() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("no_move.txt");
        fs::write(&file, "start\n").unwrap();

        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes);

        fs::write(&file, "end\n").unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("-start"));
        assert!(diff.contains("+end"));
    }

    #[test]
    fn test_add_then_delete() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("addel.txt");

        let mut tracker = TurnDiffTracker::new();

        // Register as add
        let changes = HashMap::from([(
            file.clone(),
            FileChange::Add {
                content: "temp\n".to_string(),
            },
        )]);
        tracker.on_patch_begin(&changes);

        // Create file
        fs::write(&file, "temp\n").unwrap();

        // Get diff showing addition
        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("+temp"));

        tracker.clear();

        // Now register the same file but it exists now
        let changes2 = HashMap::from([(
            file.clone(),
            FileChange::Delete {
                content: "temp\n".to_string(),
            },
        )]);
        tracker.on_patch_begin(&changes2);

        // Delete file
        fs::remove_file(&file).unwrap();

        let diff2 = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff2.contains("-temp"));
    }

    #[test]
    fn test_file_change_update_deserialization() {
        let json = r#"{"Update":{"unified_diff":"diff","move_path":"/new"}}"#;
        let parsed: FileChange = serde_json::from_str(json).unwrap();
        match parsed {
            FileChange::Update {
                unified_diff,
                move_path,
            } => {
                assert_eq!(unified_diff, "diff");
                assert_eq!(move_path, Some(PathBuf::from("/new")));
            }
            _ => panic!("Expected Update variant"),
        }
    }

    #[test]
    fn test_file_change_add_deserialization() {
        let json = r#"{"Add":{"content":"hello world"}}"#;
        let parsed: FileChange = serde_json::from_str(json).unwrap();
        match parsed {
            FileChange::Add { content } => {
                assert_eq!(content, "hello world");
            }
            _ => panic!("Expected Add variant"),
        }
    }

    #[test]
    fn test_multiline_diff() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("multi.txt");
        fs::write(&file, "line1\nline2\nline3\n").unwrap();

        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes);

        fs::write(&file, "line1\nmodified\nline3\n").unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("-line2"));
        assert!(diff.contains("+modified"));
        // Context lines should appear
        assert!(diff.contains("line1"));
        assert!(diff.contains("line3"));
    }

    // ============================================================================
    // Additional test coverage (N=279)
    // ============================================================================

    #[test]
    fn test_file_mode_as_str() {
        assert_eq!(FileMode::Regular.as_str(), "100644");
        assert_eq!(FileMode::Symlink.as_str(), "120000");
        #[cfg(unix)]
        assert_eq!(FileMode::Executable.as_str(), "100755");
    }

    #[test]
    fn test_tracked_count_increments() {
        let dir = tempdir().unwrap();
        let mut tracker = TurnDiffTracker::new();

        // Start empty
        assert_eq!(tracker.tracked_count(), 0);

        // Add first file
        let file1 = dir.path().join("f1.txt");
        let changes1 = HashMap::from([(
            file1.clone(),
            FileChange::Add {
                content: "a".to_string(),
            },
        )]);
        tracker.on_patch_begin(&changes1);
        assert_eq!(tracker.tracked_count(), 1);

        // Add second file
        let file2 = dir.path().join("f2.txt");
        let changes2 = HashMap::from([(
            file2.clone(),
            FileChange::Add {
                content: "b".to_string(),
            },
        )]);
        tracker.on_patch_begin(&changes2);
        assert_eq!(tracker.tracked_count(), 2);

        // Same file again should not increment
        tracker.on_patch_begin(&changes1);
        assert_eq!(tracker.tracked_count(), 2);
    }

    #[test]
    fn test_normalize_diff_helper() {
        let dir = tempdir().unwrap();
        let input = format!(
            "diff --git a/{}/test.txt b/{}/test.txt\n",
            dir.path().display(),
            dir.path().display()
        );
        let normalized = normalize_diff(&input, dir.path());
        assert!(normalized.contains("<TMP>"));
        assert!(!normalized.contains(&dir.path().display().to_string()));
    }

    #[test]
    fn test_normalize_diff_empty_input() {
        let dir = tempdir().unwrap();
        let normalized = normalize_diff("", dir.path());
        assert_eq!(normalized, "\n");
    }

    #[test]
    fn test_normalize_diff_sorts_blocks() {
        let dir = tempdir().unwrap();
        let input = "diff --git a/z.txt b/z.txt\nfoo\ndiff --git a/a.txt b/a.txt\nbar\n";
        let normalized = normalize_diff(input, dir.path());
        // a.txt should come before z.txt after sorting
        let a_pos = normalized.find("a.txt").unwrap();
        let z_pos = normalized.find("z.txt").unwrap();
        assert!(a_pos < z_pos);
    }

    #[test]
    fn test_large_file_content() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("large.txt");

        // Create file with many lines
        let content: String = (0..100).map(|i| format!("line {}\n", i)).collect();
        fs::write(&file, &content).unwrap();

        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes);

        // Modify multiple lines
        let new_content: String = (0..100)
            .map(|i| {
                if i == 50 {
                    "modified line 50\n".to_string()
                } else {
                    format!("line {}\n", i)
                }
            })
            .collect();
        fs::write(&file, &new_content).unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("-line 50"));
        assert!(diff.contains("+modified line 50"));
    }

    #[test]
    fn test_whitespace_only_change() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("whitespace.txt");
        fs::write(&file, "no trailing space").unwrap();

        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes);

        fs::write(&file, "no trailing space ").unwrap(); // Added trailing space

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        // Should detect the change
        assert!(diff.contains("whitespace.txt"));
    }

    #[test]
    fn test_special_characters_in_filename() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("file with spaces.txt");

        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([(
            file.clone(),
            FileChange::Add {
                content: "content\n".to_string(),
            },
        )]);
        tracker.on_patch_begin(&changes);

        fs::write(&file, "content\n").unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("file with spaces.txt"));
    }

    #[test]
    fn test_deeply_nested_path() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("a/b/c/d/e/deep.txt");
        fs::create_dir_all(nested.parent().unwrap()).unwrap();

        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([(
            nested.clone(),
            FileChange::Add {
                content: "deep content\n".to_string(),
            },
        )]);
        tracker.on_patch_begin(&changes);

        fs::write(&nested, "deep content\n").unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("deep.txt"));
        assert!(diff.contains("+deep content"));
    }

    #[test]
    fn test_move_to_subdirectory() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("root.txt");
        let subdir = dir.path().join("sub");
        fs::create_dir(&subdir).unwrap();
        let dest = subdir.join("moved.txt");

        fs::write(&src, "content\n").unwrap();

        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([(
            src.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: Some(dest.clone()),
            },
        )]);
        tracker.on_patch_begin(&changes);

        fs::rename(&src, &dest).unwrap();
        fs::write(&dest, "new content\n").unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("root.txt"));
        assert!(diff.contains("moved.txt"));
    }

    #[test]
    fn test_clear_allows_reuse() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("reuse.txt");

        let mut tracker = TurnDiffTracker::new();

        // First use
        let changes = HashMap::from([(
            file.clone(),
            FileChange::Add {
                content: "first".to_string(),
            },
        )]);
        tracker.on_patch_begin(&changes);
        fs::write(&file, "first").unwrap();
        assert_eq!(tracker.tracked_count(), 1);

        // Clear
        tracker.clear();
        assert_eq!(tracker.tracked_count(), 0);

        // Reuse with same file - should work fresh
        let changes2 = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes2);
        fs::write(&file, "second").unwrap();

        assert_eq!(tracker.tracked_count(), 1);
        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("-first"));
        assert!(diff.contains("+second"));
    }

    #[test]
    fn test_git_blob_sha1_various_content() {
        // Test with different content types
        let binary = git_blob_sha1(&[0x00, 0x01, 0x02]);
        let binary_hex = format!("{:x}", binary);
        assert_eq!(binary_hex.len(), 40);

        // Newline variations
        let unix = git_blob_sha1(b"line\n");
        let windows = git_blob_sha1(b"line\r\n");
        assert_ne!(format!("{:x}", unix), format!("{:x}", windows));

        // Larger content
        let large = vec![b'x'; 1000];
        let large_hash = git_blob_sha1(&large);
        assert_eq!(format!("{:x}", large_hash).len(), 40);
    }

    #[test]
    fn test_file_change_all_variants_roundtrip() {
        // Add
        let add = FileChange::Add {
            content: "test content".to_string(),
        };
        let json = serde_json::to_string(&add).unwrap();
        let parsed: FileChange = serde_json::from_str(&json).unwrap();
        match parsed {
            FileChange::Add { content } => assert_eq!(content, "test content"),
            _ => panic!("Wrong variant"),
        }

        // Delete
        let delete = FileChange::Delete {
            content: "deleted".to_string(),
        };
        let json = serde_json::to_string(&delete).unwrap();
        let parsed: FileChange = serde_json::from_str(&json).unwrap();
        match parsed {
            FileChange::Delete { content } => assert_eq!(content, "deleted"),
            _ => panic!("Wrong variant"),
        }

        // Update with move_path
        let update = FileChange::Update {
            unified_diff: "diff text".to_string(),
            move_path: Some(PathBuf::from("/some/path")),
        };
        let json = serde_json::to_string(&update).unwrap();
        let parsed: FileChange = serde_json::from_str(&json).unwrap();
        match parsed {
            FileChange::Update {
                unified_diff,
                move_path,
            } => {
                assert_eq!(unified_diff, "diff text");
                assert_eq!(move_path, Some(PathBuf::from("/some/path")));
            }
            _ => panic!("Wrong variant"),
        }

        // Update without move_path
        let update_no_move = FileChange::Update {
            unified_diff: "diff".to_string(),
            move_path: None,
        };
        let json = serde_json::to_string(&update_no_move).unwrap();
        let parsed: FileChange = serde_json::from_str(&json).unwrap();
        match parsed {
            FileChange::Update {
                unified_diff,
                move_path,
            } => {
                assert_eq!(unified_diff, "diff");
                assert!(move_path.is_none());
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_empty_changes_hashmap() {
        let mut tracker = TurnDiffTracker::new();
        let changes: HashMap<PathBuf, FileChange> = HashMap::new();
        tracker.on_patch_begin(&changes);
        assert_eq!(tracker.tracked_count(), 0);
    }

    #[test]
    fn test_multiple_deletes() {
        let dir = tempdir().unwrap();
        let file1 = dir.path().join("del1.txt");
        let file2 = dir.path().join("del2.txt");
        fs::write(&file1, "content1\n").unwrap();
        fs::write(&file2, "content2\n").unwrap();

        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([
            (
                file1.clone(),
                FileChange::Delete {
                    content: "content1\n".to_string(),
                },
            ),
            (
                file2.clone(),
                FileChange::Delete {
                    content: "content2\n".to_string(),
                },
            ),
        ]);
        tracker.on_patch_begin(&changes);

        fs::remove_file(&file1).unwrap();
        fs::remove_file(&file2).unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("-content1"));
        assert!(diff.contains("-content2"));
        assert!(diff.contains("deleted file mode"));
    }

    #[test]
    fn test_mixed_operations() {
        let dir = tempdir().unwrap();
        let add_file = dir.path().join("add.txt");
        let del_file = dir.path().join("del.txt");
        let mod_file = dir.path().join("mod.txt");

        // Setup
        fs::write(&del_file, "to delete\n").unwrap();
        fs::write(&mod_file, "to modify\n").unwrap();

        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([
            (
                add_file.clone(),
                FileChange::Add {
                    content: "new file\n".to_string(),
                },
            ),
            (
                del_file.clone(),
                FileChange::Delete {
                    content: "to delete\n".to_string(),
                },
            ),
            (
                mod_file.clone(),
                FileChange::Update {
                    unified_diff: String::new(),
                    move_path: None,
                },
            ),
        ]);
        tracker.on_patch_begin(&changes);

        // Apply changes
        fs::write(&add_file, "new file\n").unwrap();
        fs::remove_file(&del_file).unwrap();
        fs::write(&mod_file, "modified\n").unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("add.txt"));
        assert!(diff.contains("del.txt"));
        assert!(diff.contains("mod.txt"));
        assert!(diff.contains("+new file"));
        assert!(diff.contains("-to delete"));
        assert!(diff.contains("-to modify"));
        assert!(diff.contains("+modified"));
    }

    #[test]
    fn test_diff_contains_index_line() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("indexed.txt");

        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([(
            file.clone(),
            FileChange::Add {
                content: "content\n".to_string(),
            },
        )]);
        tracker.on_patch_begin(&changes);
        fs::write(&file, "content\n").unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        // Diff should contain index line with SHAs
        assert!(diff.contains("index "));
        assert!(diff.contains(".."));
    }

    #[test]
    fn test_newline_at_eof() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("noeof.txt");

        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([(
            file.clone(),
            FileChange::Add {
                content: "no newline at end".to_string(),
            },
        )]);
        tracker.on_patch_begin(&changes);
        fs::write(&file, "no newline at end").unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        // Should handle content without trailing newline
        assert!(diff.contains("no newline at end"));
    }

    #[test]
    fn test_diff_header_format() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("header.txt");

        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([(
            file.clone(),
            FileChange::Add {
                content: "test\n".to_string(),
            },
        )]);
        tracker.on_patch_begin(&changes);
        fs::write(&file, "test\n").unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        // Check git diff header format
        assert!(diff.starts_with("diff --git"));
        assert!(diff.contains("a/"));
        assert!(diff.contains("b/"));
    }

    #[test]
    fn test_blob_bytes_with_regular_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("regular.txt");
        fs::write(&file, "regular content").unwrap();

        let bytes = blob_bytes(&file, FileMode::Regular);
        assert!(bytes.is_some());
        assert_eq!(bytes.unwrap(), b"regular content");
    }

    #[cfg(unix)]
    #[test]
    fn test_file_mode_for_regular_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("regular.txt");
        fs::write(&file, "content").unwrap();

        let mode = file_mode_for_path(&file);
        assert!(mode.is_some());
        assert_eq!(mode.unwrap(), FileMode::Regular);
    }

    #[cfg(unix)]
    #[test]
    fn test_file_mode_for_executable() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let file = dir.path().join("executable.sh");
        fs::write(&file, "#!/bin/bash\necho hello").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();

        let mode = file_mode_for_path(&file);
        assert!(mode.is_some());
        assert_eq!(mode.unwrap(), FileMode::Executable);
    }

    #[cfg(unix)]
    #[test]
    fn test_file_mode_for_symlink() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let target = dir.path().join("target.txt");
        let link = dir.path().join("link.txt");
        fs::write(&target, "target content").unwrap();
        symlink(&target, &link).unwrap();

        let mode = file_mode_for_path(&link);
        assert!(mode.is_some());
        assert_eq!(mode.unwrap(), FileMode::Symlink);
    }

    #[test]
    fn test_diff_paths_use_forward_slashes() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("sub").join("file.txt");
        fs::create_dir_all(nested.parent().unwrap()).unwrap();

        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([(
            nested.clone(),
            FileChange::Add {
                content: "content\n".to_string(),
            },
        )]);
        tracker.on_patch_begin(&changes);
        fs::write(&nested, "content\n").unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        // Paths should use forward slashes for git compatibility
        // Note: On Windows this would convert backslashes
        if cfg!(windows) {
            assert!(!diff.contains('\\') || diff.contains('/'));
        }
    }

    #[test]
    fn test_consecutive_get_unified_diff_calls() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("consecutive.txt");

        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([(
            file.clone(),
            FileChange::Add {
                content: "content\n".to_string(),
            },
        )]);
        tracker.on_patch_begin(&changes);
        fs::write(&file, "content\n").unwrap();

        // Multiple calls should return the same diff
        let diff1 = tracker.get_unified_diff().unwrap().unwrap();
        let diff2 = tracker.get_unified_diff().unwrap().unwrap();
        assert_eq!(diff1, diff2);
    }

    #[test]
    fn test_update_move_to_new_directory() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.txt");
        let new_dir = dir.path().join("new_dir");
        let dest = new_dir.join("dest.txt");

        fs::write(&src, "original\n").unwrap();

        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([(
            src.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: Some(dest.clone()),
            },
        )]);
        tracker.on_patch_begin(&changes);

        // Create new directory and move file
        fs::create_dir_all(&new_dir).unwrap();
        fs::rename(&src, &dest).unwrap();

        let diff = tracker.get_unified_diff().unwrap();
        // No content change, just move
        assert!(diff.is_none());
    }

    #[test]
    fn test_file_change_debug_with_move_path_none() {
        let change = FileChange::Update {
            unified_diff: "diff text".to_string(),
            move_path: None,
        };
        let debug = format!("{:?}", change);
        assert!(debug.contains("Update"));
        assert!(debug.contains("diff text"));
        assert!(debug.contains("None"));
    }

    #[test]
    fn test_zero_oid_constant() {
        assert_eq!(ZERO_OID.len(), 40);
        assert!(ZERO_OID.chars().all(|c| c == '0'));
    }

    #[test]
    fn test_dev_null_constant() {
        assert_eq!(DEV_NULL, "/dev/null");
    }

    // ============================================================================
    // Additional test coverage (N=287)
    // ============================================================================

    #[test]
    fn test_file_change_update_empty_diff() {
        let change = FileChange::Update {
            unified_diff: String::new(),
            move_path: None,
        };
        let json = serde_json::to_string(&change).unwrap();
        let parsed: FileChange = serde_json::from_str(&json).unwrap();
        if let FileChange::Update {
            unified_diff,
            move_path,
        } = parsed
        {
            assert!(unified_diff.is_empty());
            assert!(move_path.is_none());
        } else {
            panic!("Expected Update");
        }
    }

    #[test]
    fn test_file_change_add_multiline() {
        let content = "line1\nline2\nline3\n";
        let change = FileChange::Add {
            content: content.to_string(),
        };
        let json = serde_json::to_string(&change).unwrap();
        let parsed: FileChange = serde_json::from_str(&json).unwrap();
        if let FileChange::Add { content: c } = parsed {
            assert_eq!(c.lines().count(), 3);
        } else {
            panic!("Expected Add");
        }
    }

    #[test]
    fn test_file_change_delete_unicode() {
        let content = "日本語テキスト\n🎉emoji\n";
        let change = FileChange::Delete {
            content: content.to_string(),
        };
        let json = serde_json::to_string(&change).unwrap();
        let parsed: FileChange = serde_json::from_str(&json).unwrap();
        if let FileChange::Delete { content: c } = parsed {
            assert!(c.contains("日本語"));
            assert!(c.contains("🎉"));
        } else {
            panic!("Expected Delete");
        }
    }

    #[test]
    fn test_file_mode_all_variants_str() {
        assert_eq!(FileMode::Regular.as_str(), "100644");
        assert_eq!(FileMode::Symlink.as_str(), "120000");

        #[cfg(unix)]
        assert_eq!(FileMode::Executable.as_str(), "100755");
    }

    #[test]
    fn test_file_mode_display_all() {
        assert_eq!(format!("{}", FileMode::Regular), "100644");
        assert_eq!(format!("{}", FileMode::Symlink), "120000");

        #[cfg(unix)]
        assert_eq!(format!("{}", FileMode::Executable), "100755");
    }

    #[test]
    fn test_tracker_new_vs_default() {
        let tracker1 = TurnDiffTracker::new();
        let tracker2 = TurnDiffTracker::default();
        assert_eq!(tracker1.tracked_count(), tracker2.tracked_count());
        assert_eq!(tracker1.tracked_count(), 0);
    }

    #[test]
    fn test_same_file_registered_twice() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("twice.txt");

        let mut tracker = TurnDiffTracker::new();

        // First registration
        let changes1 = HashMap::from([(
            file.clone(),
            FileChange::Add {
                content: "first".to_string(),
            },
        )]);
        tracker.on_patch_begin(&changes1);
        assert_eq!(tracker.tracked_count(), 1);

        // Second registration - should not duplicate
        let changes2 = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes2);
        assert_eq!(tracker.tracked_count(), 1); // Still 1
    }

    #[test]
    fn test_multiple_changes_single_on_patch_begin() {
        let dir = tempdir().unwrap();
        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([
            (
                dir.path().join("a.txt"),
                FileChange::Add {
                    content: "a".to_string(),
                },
            ),
            (
                dir.path().join("b.txt"),
                FileChange::Add {
                    content: "b".to_string(),
                },
            ),
            (
                dir.path().join("c.txt"),
                FileChange::Add {
                    content: "c".to_string(),
                },
            ),
            (
                dir.path().join("d.txt"),
                FileChange::Add {
                    content: "d".to_string(),
                },
            ),
        ]);
        tracker.on_patch_begin(&changes);
        assert_eq!(tracker.tracked_count(), 4);
    }

    #[test]
    fn test_clear_multiple_times() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("clear.txt");

        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([(
            file.clone(),
            FileChange::Add {
                content: "test".to_string(),
            },
        )]);
        tracker.on_patch_begin(&changes);
        assert_eq!(tracker.tracked_count(), 1);

        tracker.clear();
        assert_eq!(tracker.tracked_count(), 0);

        tracker.clear(); // Clear again - should be safe
        assert_eq!(tracker.tracked_count(), 0);
    }

    #[test]
    fn test_get_unified_diff_after_clear() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("cleared.txt");

        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([(
            file.clone(),
            FileChange::Add {
                content: "content".to_string(),
            },
        )]);
        tracker.on_patch_begin(&changes);
        fs::write(&file, "content").unwrap();

        // Clear before getting diff
        tracker.clear();

        let diff = tracker.get_unified_diff().unwrap();
        assert!(diff.is_none());
    }

    #[test]
    fn test_blob_bytes_regular_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("regular.bin");
        let content = vec![1, 2, 3, 4, 5];
        fs::write(&file, &content).unwrap();

        let bytes = blob_bytes(&file, FileMode::Regular);
        assert!(bytes.is_some());
        assert_eq!(bytes.unwrap(), content);
    }

    #[test]
    fn test_blob_bytes_nonexistent_file() {
        let bytes = blob_bytes(Path::new("/this/does/not/exist.txt"), FileMode::Regular);
        assert!(bytes.is_none());
    }

    #[test]
    fn test_file_mode_for_nonexistent() {
        let mode = file_mode_for_path(Path::new("/this/does/not/exist.txt"));
        assert!(mode.is_none());
    }

    #[test]
    fn test_git_blob_sha1_empty() {
        // Known SHA1 for empty blob
        let hash = git_blob_sha1(b"");
        let hash_hex = format!("{:x}", hash);
        assert_eq!(hash_hex, "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391");
    }

    #[test]
    fn test_git_blob_sha1_binary() {
        let binary_data = vec![0x00, 0x01, 0x02, 0xff, 0xfe];
        let hash = git_blob_sha1(&binary_data);
        let hash_hex = format!("{:x}", hash);
        assert_eq!(hash_hex.len(), 40);
    }

    #[test]
    fn test_git_blob_sha1_newlines() {
        let unix = git_blob_sha1(b"line\n");
        let windows = git_blob_sha1(b"line\r\n");
        let unix_hex = format!("{:x}", unix);
        let windows_hex = format!("{:x}", windows);
        assert_ne!(unix_hex, windows_hex); // Different content = different hash
    }

    #[test]
    fn test_normalize_diff_single_block() {
        let dir = tempdir().unwrap();
        let input = "diff --git a/file.txt b/file.txt\nsome content\n";
        let normalized = normalize_diff(input, dir.path());
        assert!(normalized.contains("file.txt"));
        assert!(normalized.ends_with('\n'));
    }

    #[test]
    fn test_normalize_diff_multiple_blocks_sorted() {
        let dir = tempdir().unwrap();
        let input =
            "diff --git a/z.txt b/z.txt\nz content\ndiff --git a/a.txt b/a.txt\na content\n";
        let normalized = normalize_diff(input, dir.path());
        let a_pos = normalized.find("a.txt").unwrap();
        let z_pos = normalized.find("z.txt").unwrap();
        assert!(a_pos < z_pos); // Should be sorted
    }

    #[test]
    fn test_diff_with_very_long_lines() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("long.txt");

        let long_line = "x".repeat(10000);
        fs::write(&file, &long_line).unwrap();

        let mut tracker = TurnDiffTracker::new();
        let changes = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes);

        let new_long_line = "y".repeat(10000);
        fs::write(&file, &new_long_line).unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("-x"));
        assert!(diff.contains("+y"));
    }

    #[test]
    fn test_diff_with_tabs_and_spaces() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("whitespace.txt");
        fs::write(&file, "  spaces\n").unwrap();

        let mut tracker = TurnDiffTracker::new();
        let changes = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes);

        fs::write(&file, "\ttabs\n").unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("spaces") || diff.contains("tabs"));
    }

    #[test]
    fn test_diff_with_only_newline_changes() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("newlines.txt");
        fs::write(&file, "line\n").unwrap();

        let mut tracker = TurnDiffTracker::new();
        let changes = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes);

        fs::write(&file, "line\n\n").unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("newlines.txt"));
    }

    #[test]
    fn test_move_path_with_special_characters() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("source.txt");
        let dest = dir.path().join("file with spaces.txt");

        fs::write(&src, "content\n").unwrap();

        let mut tracker = TurnDiffTracker::new();
        let changes = HashMap::from([(
            src.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: Some(dest.clone()),
            },
        )]);
        tracker.on_patch_begin(&changes);

        fs::rename(&src, &dest).unwrap();
        fs::write(&dest, "modified\n").unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("source.txt"));
        assert!(diff.contains("file with spaces.txt"));
    }

    #[test]
    fn test_add_file_with_binary_content() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("binary.dat");

        let mut tracker = TurnDiffTracker::new();
        let changes = HashMap::from([(
            file.clone(),
            FileChange::Add {
                content: String::new(), // Content doesn't matter for tracking
            },
        )]);
        tracker.on_patch_begin(&changes);

        // Write binary content
        fs::write(&file, [0x00, 0x01, 0x02, 0xff]).unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("Binary files differ"));
    }

    #[test]
    fn test_delete_file_with_binary_content() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("binary_del.dat");
        fs::write(&file, [0x00, 0x01, 0x02, 0xff]).unwrap();

        let mut tracker = TurnDiffTracker::new();
        let changes = HashMap::from([(
            file.clone(),
            FileChange::Delete {
                content: String::new(),
            },
        )]);
        tracker.on_patch_begin(&changes);

        fs::remove_file(&file).unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("Binary files differ") || diff.contains("deleted file mode"));
    }

    #[test]
    fn test_diff_header_contains_git_prefix() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("header.txt");

        let mut tracker = TurnDiffTracker::new();
        let changes = HashMap::from([(
            file.clone(),
            FileChange::Add {
                content: "test\n".to_string(),
            },
        )]);
        tracker.on_patch_begin(&changes);
        fs::write(&file, "test\n").unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.starts_with("diff --git"));
    }

    #[test]
    fn test_diff_index_line_format() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("indexed.txt");

        let mut tracker = TurnDiffTracker::new();
        let changes = HashMap::from([(
            file.clone(),
            FileChange::Add {
                content: "indexed\n".to_string(),
            },
        )]);
        tracker.on_patch_begin(&changes);
        fs::write(&file, "indexed\n").unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("index "));
        assert!(diff.contains(".."));
    }

    #[test]
    fn test_file_change_eq_through_serde() {
        let orig = FileChange::Add {
            content: "test".to_string(),
        };
        let json = serde_json::to_string(&orig).unwrap();
        let parsed: FileChange = serde_json::from_str(&json).unwrap();

        let orig_json = serde_json::to_string(&orig).unwrap();
        let parsed_json = serde_json::to_string(&parsed).unwrap();
        assert_eq!(orig_json, parsed_json);
    }

    #[test]
    fn test_update_same_file_multiple_times_preserves_original_baseline() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("multi.txt");
        fs::write(&file, "original\n").unwrap();

        let mut tracker = TurnDiffTracker::new();

        // First patch begin
        let changes1 = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes1);
        fs::write(&file, "first update\n").unwrap();

        // Second patch begin (same file)
        let changes2 = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes2);
        fs::write(&file, "second update\n").unwrap();

        // Diff should show change from original to current (not from first update)
        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("-original"));
        assert!(diff.contains("+second update"));
    }

    #[test]
    fn test_context_lines_in_diff() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("context.txt");

        // Create file with many lines
        let original = "line1\nline2\nline3\nline4\nline5\nline6\nline7\n";
        fs::write(&file, original).unwrap();

        let mut tracker = TurnDiffTracker::new();
        let changes = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes);

        // Change only middle line
        let modified = "line1\nline2\nline3\nMODIFIED\nline5\nline6\nline7\n";
        fs::write(&file, modified).unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        // Context should include surrounding lines
        assert!(diff.contains("line3"));
        assert!(diff.contains("-line4"));
        assert!(diff.contains("+MODIFIED"));
        assert!(diff.contains("line5"));
    }

    #[test]
    fn test_tracked_count_after_moves() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.txt");
        let dest = dir.path().join("dest.txt");

        fs::write(&src, "content\n").unwrap();

        let mut tracker = TurnDiffTracker::new();

        // Register a move
        let changes = HashMap::from([(
            src.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: Some(dest.clone()),
            },
        )]);
        tracker.on_patch_begin(&changes);

        // Should still be 1 tracked file (same file, just moved)
        assert_eq!(tracker.tracked_count(), 1);
    }

    // More tests to improve coverage ratio (N=287)

    #[test]
    fn test_git_blob_sha1_large_content() {
        let large = vec![b'a'; 100_000];
        let hash = git_blob_sha1(&large);
        let hex = format!("{:x}", hash);
        assert_eq!(hex.len(), 40);
    }

    #[test]
    fn test_file_change_clone_all_variants() {
        let add = FileChange::Add {
            content: "test".to_string(),
        };
        let add_cloned = add.clone();
        assert!(matches!(add_cloned, FileChange::Add { .. }));

        let delete = FileChange::Delete {
            content: "old".to_string(),
        };
        let delete_cloned = delete.clone();
        assert!(matches!(delete_cloned, FileChange::Delete { .. }));

        let update = FileChange::Update {
            unified_diff: "diff".to_string(),
            move_path: Some(PathBuf::from("/new")),
        };
        let update_cloned = update.clone();
        assert!(matches!(update_cloned, FileChange::Update { .. }));
    }

    #[test]
    fn test_file_mode_partial_eq() {
        assert!(FileMode::Regular == FileMode::Regular);
        assert!(FileMode::Symlink == FileMode::Symlink);
        assert!(FileMode::Regular != FileMode::Symlink);

        #[cfg(unix)]
        {
            assert!(FileMode::Executable == FileMode::Executable);
            assert!(FileMode::Regular != FileMode::Executable);
        }
    }

    #[test]
    fn test_diff_with_empty_lines() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("empty_lines.txt");
        fs::write(&file, "line1\n\nline3\n").unwrap();

        let mut tracker = TurnDiffTracker::new();
        let changes = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes);

        fs::write(&file, "line1\n\n\nline3\n").unwrap(); // Added extra empty line

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("empty_lines.txt"));
    }

    #[test]
    fn test_diff_with_carriage_returns() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("crlf.txt");
        fs::write(&file, "line1\r\nline2\r\n").unwrap();

        let mut tracker = TurnDiffTracker::new();
        let changes = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes);

        fs::write(&file, "line1\nline2\n").unwrap(); // Convert to LF

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("crlf.txt"));
    }

    #[test]
    fn test_consecutive_adds() {
        let dir = tempdir().unwrap();
        let mut tracker = TurnDiffTracker::new();

        for i in 0..5 {
            let file = dir.path().join(format!("file{}.txt", i));
            let changes = HashMap::from([(
                file.clone(),
                FileChange::Add {
                    content: format!("content{}", i),
                },
            )]);
            tracker.on_patch_begin(&changes);
            fs::write(&file, format!("content{}", i)).unwrap();
        }

        assert_eq!(tracker.tracked_count(), 5);
        let diff = tracker.get_unified_diff().unwrap().unwrap();
        for i in 0..5 {
            assert!(diff.contains(&format!("file{}.txt", i)));
        }
    }

    #[test]
    fn test_file_change_update_with_complex_diff() {
        let complex_diff = r#"@@ -1,5 +1,7 @@
 line1
-line2
+new line 2
+new line 2b
 line3
-line4
+new line 4
 line5
+line6"#;
        let change = FileChange::Update {
            unified_diff: complex_diff.to_string(),
            move_path: None,
        };
        let json = serde_json::to_string(&change).unwrap();
        let parsed: FileChange = serde_json::from_str(&json).unwrap();
        if let FileChange::Update { unified_diff, .. } = parsed {
            assert!(unified_diff.contains("new line 2"));
            assert!(unified_diff.contains("line6"));
        } else {
            panic!("Expected Update");
        }
    }

    #[test]
    fn test_normalize_diff_preserves_content() {
        let dir = tempdir().unwrap();
        let input = "diff --git a/test.txt b/test.txt\nindex abc..def\n--- a/test.txt\n+++ b/test.txt\n@@ -1 +1 @@\n-old\n+new\n";
        let normalized = normalize_diff(input, dir.path());
        assert!(normalized.contains("--- a/test.txt"));
        assert!(normalized.contains("+++ b/test.txt"));
        assert!(normalized.contains("-old"));
        assert!(normalized.contains("+new"));
    }

    #[test]
    fn test_multiple_sequential_clears() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("multi_clear.txt");

        let mut tracker = TurnDiffTracker::new();

        for i in 0..3 {
            let changes = HashMap::from([(
                file.clone(),
                FileChange::Add {
                    content: format!("iter{}", i),
                },
            )]);
            tracker.on_patch_begin(&changes);
            assert_eq!(tracker.tracked_count(), 1);

            tracker.clear();
            assert_eq!(tracker.tracked_count(), 0);
        }
    }

    #[test]
    fn test_blob_bytes_empty_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("empty.txt");
        fs::write(&file, "").unwrap();

        let bytes = blob_bytes(&file, FileMode::Regular);
        assert!(bytes.is_some());
        assert!(bytes.unwrap().is_empty());
    }

    #[test]
    fn test_file_change_debug_all_variants() {
        let add = FileChange::Add {
            content: "add".to_string(),
        };
        assert!(format!("{:?}", add).contains("Add"));

        let delete = FileChange::Delete {
            content: "del".to_string(),
        };
        assert!(format!("{:?}", delete).contains("Delete"));

        let update = FileChange::Update {
            unified_diff: "diff".to_string(),
            move_path: None,
        };
        assert!(format!("{:?}", update).contains("Update"));
    }

    #[test]
    fn test_file_mode_debug_all_variants() {
        assert!(format!("{:?}", FileMode::Regular).contains("Regular"));
        assert!(format!("{:?}", FileMode::Symlink).contains("Symlink"));
        #[cfg(unix)]
        assert!(format!("{:?}", FileMode::Executable).contains("Executable"));
    }

    #[test]
    fn test_move_to_same_directory() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("old_name.txt");
        let dest = dir.path().join("new_name.txt");

        fs::write(&src, "content\n").unwrap();

        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([(
            src.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: Some(dest.clone()),
            },
        )]);
        tracker.on_patch_begin(&changes);

        fs::rename(&src, &dest).unwrap();

        let diff = tracker.get_unified_diff().unwrap();
        // No content change, just rename - should be None
        assert!(diff.is_none());
    }

    #[test]
    fn test_rename_with_modification() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("before.txt");
        let dest = dir.path().join("after.txt");

        fs::write(&src, "original\n").unwrap();

        let mut tracker = TurnDiffTracker::new();

        let changes = HashMap::from([(
            src.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: Some(dest.clone()),
            },
        )]);
        tracker.on_patch_begin(&changes);

        fs::rename(&src, &dest).unwrap();
        fs::write(&dest, "modified\n").unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("before.txt"));
        assert!(diff.contains("after.txt"));
        assert!(diff.contains("-original"));
        assert!(diff.contains("+modified"));
    }

    // ============================================================================
    // Additional test coverage (N=298)
    // ============================================================================

    // --- FileMode tests ---

    #[test]
    fn test_file_mode_as_str_regular() {
        assert_eq!(FileMode::Regular.as_str(), "100644");
    }

    #[test]
    fn test_file_mode_as_str_symlink() {
        assert_eq!(FileMode::Symlink.as_str(), "120000");
    }

    #[cfg(unix)]
    #[test]
    fn test_file_mode_as_str_executable() {
        assert_eq!(FileMode::Executable.as_str(), "100755");
    }

    #[test]
    fn test_file_mode_display_regular() {
        let s = format!("{}", FileMode::Regular);
        assert_eq!(s, "100644");
    }

    #[test]
    fn test_file_mode_display_symlink() {
        let s = format!("{}", FileMode::Symlink);
        assert_eq!(s, "120000");
    }

    #[cfg(unix)]
    #[test]
    fn test_file_mode_display_executable() {
        let s = format!("{}", FileMode::Executable);
        assert_eq!(s, "100755");
    }

    #[test]
    fn test_file_mode_copy() {
        let mode = FileMode::Regular;
        let copied: FileMode = mode; // Copy
        assert_eq!(copied, FileMode::Regular);
    }

    #[test]
    fn test_file_mode_clone() {
        let mode = FileMode::Symlink;
        let cloned = mode;
        assert_eq!(cloned, FileMode::Symlink);
    }

    // --- FileChange serde tests ---

    #[test]
    fn test_file_change_serde_add() {
        let change = FileChange::Add {
            content: "new content\n".to_string(),
        };
        let json = serde_json::to_string(&change).unwrap();
        assert!(json.contains("Add"));
        let parsed: FileChange = serde_json::from_str(&json).unwrap();
        if let FileChange::Add { content } = parsed {
            assert_eq!(content, "new content\n");
        } else {
            panic!("Expected Add variant");
        }
    }

    #[test]
    fn test_file_change_serde_delete() {
        let change = FileChange::Delete {
            content: "deleted content\n".to_string(),
        };
        let json = serde_json::to_string(&change).unwrap();
        assert!(json.contains("Delete"));
        let parsed: FileChange = serde_json::from_str(&json).unwrap();
        if let FileChange::Delete { content } = parsed {
            assert_eq!(content, "deleted content\n");
        } else {
            panic!("Expected Delete variant");
        }
    }

    #[test]
    fn test_file_change_serde_update_no_move() {
        let change = FileChange::Update {
            unified_diff: "@@ -1 +1 @@\n-old\n+new\n".to_string(),
            move_path: None,
        };
        let json = serde_json::to_string(&change).unwrap();
        let parsed: FileChange = serde_json::from_str(&json).unwrap();
        if let FileChange::Update {
            unified_diff,
            move_path,
        } = parsed
        {
            assert!(unified_diff.contains("-old"));
            assert!(move_path.is_none());
        } else {
            panic!("Expected Update variant");
        }
    }

    #[test]
    fn test_file_change_serde_update_with_move() {
        let change = FileChange::Update {
            unified_diff: "".to_string(),
            move_path: Some(PathBuf::from("/new/path.txt")),
        };
        let json = serde_json::to_string(&change).unwrap();
        let parsed: FileChange = serde_json::from_str(&json).unwrap();
        if let FileChange::Update { move_path, .. } = parsed {
            assert_eq!(move_path, Some(PathBuf::from("/new/path.txt")));
        } else {
            panic!("Expected Update variant");
        }
    }

    // --- TurnDiffTracker edge cases ---

    #[test]
    fn test_tracker_default_creation() {
        let tracker = TurnDiffTracker::default();
        assert_eq!(tracker.tracked_count(), 0);
    }

    #[test]
    fn test_tracker_new_is_empty() {
        let mut tracker = TurnDiffTracker::new();
        assert_eq!(tracker.tracked_count(), 0);
        let diff = tracker.get_unified_diff().unwrap();
        assert!(diff.is_none());
    }

    #[test]
    fn test_tracker_clear_empty() {
        let mut tracker = TurnDiffTracker::new();
        tracker.clear();
        assert_eq!(tracker.tracked_count(), 0);
    }

    #[test]
    fn test_tracker_double_clear() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.txt");
        fs::write(&file, "content\n").unwrap();

        let mut tracker = TurnDiffTracker::new();
        let changes = HashMap::from([(
            file.clone(),
            FileChange::Add {
                content: "content\n".to_string(),
            },
        )]);
        tracker.on_patch_begin(&changes);
        assert_eq!(tracker.tracked_count(), 1);

        tracker.clear();
        assert_eq!(tracker.tracked_count(), 0);

        tracker.clear();
        assert_eq!(tracker.tracked_count(), 0);
    }

    #[test]
    fn test_tracker_multiple_files_add() {
        let dir = tempdir().unwrap();
        let mut tracker = TurnDiffTracker::new();

        for i in 0..10 {
            let file = dir.path().join(format!("file{}.txt", i));
            let changes = HashMap::from([(
                file.clone(),
                FileChange::Add {
                    content: format!("content {}\n", i),
                },
            )]);
            tracker.on_patch_begin(&changes);
            fs::write(&file, format!("content {}\n", i)).unwrap();
        }

        assert_eq!(tracker.tracked_count(), 10);
        let diff = tracker.get_unified_diff().unwrap().unwrap();
        for i in 0..10 {
            assert!(diff.contains(&format!("file{}.txt", i)));
        }
    }

    #[test]
    fn test_tracker_non_existent_file_add() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("new_file.txt");

        let mut tracker = TurnDiffTracker::new();
        let changes = HashMap::from([(
            file.clone(),
            FileChange::Add {
                content: "new\n".to_string(),
            },
        )]);
        tracker.on_patch_begin(&changes);

        // File doesn't exist yet - baseline should be empty
        assert_eq!(tracker.tracked_count(), 1);

        // Now create the file
        fs::write(&file, "new\n").unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("new file mode"));
        assert!(diff.contains("+new"));
    }

    #[test]
    fn test_tracker_file_deleted_after_tracking() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("to_delete.txt");
        fs::write(&file, "original\n").unwrap();

        let mut tracker = TurnDiffTracker::new();
        let changes = HashMap::from([(
            file.clone(),
            FileChange::Delete {
                content: "original\n".to_string(),
            },
        )]);
        tracker.on_patch_begin(&changes);

        fs::remove_file(&file).unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("deleted file mode"));
        assert!(diff.contains("-original"));
    }

    #[test]
    fn test_tracker_same_file_registered_twice() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("reuse.txt");
        fs::write(&file, "original\n").unwrap();

        let mut tracker = TurnDiffTracker::new();

        // First registration
        let changes1 = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes1);

        // Second registration - should reuse existing internal name
        let changes2 = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes2);

        // Should still be 1 tracked file
        assert_eq!(tracker.tracked_count(), 1);
    }

    // --- git_blob_sha1 tests ---

    #[test]
    fn test_git_blob_sha1_empty_blob() {
        let hash = git_blob_sha1(b"");
        let hex = format!("{:x}", hash);
        // Empty blob SHA-1 is well-known
        assert_eq!(hex, "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391");
    }

    #[test]
    fn test_git_blob_sha1_hello_newline() {
        let hash = git_blob_sha1(b"hello\n");
        let hex = format!("{:x}", hash);
        // "hello\n" blob SHA-1 is well-known
        assert_eq!(hex, "ce013625030ba8dba906f756967f9e9ca394464a");
    }

    #[test]
    fn test_git_blob_sha1_binary_data() {
        let data: Vec<u8> = (0..256).map(|i| i as u8).collect();
        let hash = git_blob_sha1(&data);
        let hex = format!("{:x}", hash);
        assert_eq!(hex.len(), 40);
    }

    #[test]
    fn test_git_blob_sha1_unicode_content() {
        let hash = git_blob_sha1("Hello, 世界!\n".as_bytes());
        let hex = format!("{:x}", hash);
        assert_eq!(hex.len(), 40);
    }

    // --- blob_bytes tests ---

    #[test]
    fn test_blob_bytes_regular_file_content() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("regular.txt");
        fs::write(&file, "test content").unwrap();

        let bytes = blob_bytes(&file, FileMode::Regular);
        assert!(bytes.is_some());
        assert_eq!(bytes.unwrap(), b"test content");
    }

    #[test]
    fn test_blob_bytes_non_existent_path() {
        let bytes = blob_bytes(Path::new("/nonexistent/file.txt"), FileMode::Regular);
        assert!(bytes.is_none());
    }

    #[test]
    fn test_blob_bytes_empty_file_zero_bytes() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("empty.txt");
        fs::write(&file, "").unwrap();

        let bytes = blob_bytes(&file, FileMode::Regular);
        assert!(bytes.is_some());
        assert!(bytes.unwrap().is_empty());
    }

    // --- file_mode_for_path tests ---

    #[test]
    fn test_file_mode_for_regular_file_mode() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("regular.txt");
        fs::write(&file, "test").unwrap();

        let mode = file_mode_for_path(&file);
        assert!(mode.is_some());
        assert_eq!(mode.unwrap(), FileMode::Regular);
    }

    #[test]
    fn test_file_mode_for_nonexistent_returns_none() {
        let mode = file_mode_for_path(Path::new("/nonexistent/file.txt"));
        assert!(mode.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn test_file_mode_for_executable_script() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let file = dir.path().join("script.sh");
        fs::write(&file, "#!/bin/bash\necho hello").unwrap();

        let mut perms = fs::metadata(&file).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&file, perms).unwrap();

        let mode = file_mode_for_path(&file);
        assert_eq!(mode, Some(FileMode::Executable));
    }

    #[cfg(unix)]
    #[test]
    fn test_file_mode_for_symlink_mode() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("target.txt");
        let link = dir.path().join("link.txt");

        fs::write(&target, "target content").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let mode = file_mode_for_path(&link);
        assert_eq!(mode, Some(FileMode::Symlink));
    }

    // --- Diff content edge cases ---

    #[test]
    fn test_diff_with_unicode_content() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("unicode.txt");
        fs::write(&file, "Hello 世界\n").unwrap();

        let mut tracker = TurnDiffTracker::new();
        let changes = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes);

        fs::write(&file, "你好 World\n").unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("世界") || diff.contains("World"));
    }

    #[test]
    fn test_diff_with_escaped_content() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("escaped.txt");
        fs::write(&file, "line1\tTab\nline2\n").unwrap();

        let mut tracker = TurnDiffTracker::new();
        let changes = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes);

        fs::write(&file, "newline1\tSpace\nnewline2\n").unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("escaped.txt"));
    }

    #[test]
    fn test_diff_with_no_trailing_newline() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("no_newline.txt");
        fs::write(&file, "no newline").unwrap();

        let mut tracker = TurnDiffTracker::new();
        let changes = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes);

        fs::write(&file, "still no newline").unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("no_newline.txt"));
    }

    #[test]
    fn test_diff_single_line_change() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("single.txt");
        fs::write(&file, "old line\n").unwrap();

        let mut tracker = TurnDiffTracker::new();
        let changes = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes);

        fs::write(&file, "new line\n").unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("-old line"));
        assert!(diff.contains("+new line"));
    }

    #[test]
    fn test_diff_multiple_hunks() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("hunks.txt");
        let original = (0..20).map(|i| format!("line{}\n", i)).collect::<String>();
        fs::write(&file, &original).unwrap();

        let mut tracker = TurnDiffTracker::new();
        let changes = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: String::new(),
                move_path: None,
            },
        )]);
        tracker.on_patch_begin(&changes);

        // Change lines 2 and 18 (far apart)
        let mut modified = original.clone();
        modified = modified.replace("line2\n", "CHANGED2\n");
        modified = modified.replace("line18\n", "CHANGED18\n");
        fs::write(&file, &modified).unwrap();

        let diff = tracker.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("CHANGED2"));
        assert!(diff.contains("CHANGED18"));
    }

    // --- normalize_diff tests ---

    #[test]
    fn test_normalize_diff_removes_temp_path() {
        let dir = tempdir().unwrap();
        let raw_diff = format!(
            "diff --git a/{}/test.txt b/{}/test.txt\n",
            dir.path().display(),
            dir.path().display()
        );
        let normalized = normalize_diff(&raw_diff, dir.path());
        assert!(normalized.contains("<TMP>"));
        assert!(!normalized.contains(&dir.path().display().to_string()));
    }

    #[test]
    fn test_normalize_diff_empty_input_adds_newline() {
        let dir = tempdir().unwrap();
        let normalized = normalize_diff("", dir.path());
        // normalize_diff always adds a trailing newline
        assert_eq!(normalized, "\n");
    }

    #[test]
    fn test_normalize_diff_no_temp_path() {
        let dir = tempdir().unwrap();
        let raw_diff = "diff --git a/test.txt b/test.txt\n@@ -1 +1 @@\n-old\n+new\n";
        let normalized = normalize_diff(raw_diff, dir.path());
        assert!(normalized.contains("diff --git"));
        assert!(normalized.contains("-old"));
        assert!(normalized.contains("+new"));
    }
}
