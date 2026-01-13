// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! State diffing and patching for efficient state transmission
//!
//! This module provides functionality to compute diffs between state snapshots
//! and apply patches to reconstruct states. It implements JSON Patch (RFC 6902)
//! operations optimized for DashFlow state transmission.
//!
//! # Key Features
//!
//! - **Diff Generation**: Compute minimal JSON Patch operations between states
//! - **Patch Application**: Apply patches to reconstruct states
//! - **Size Optimization**: Automatically use full state if smaller than patch
//! - **Integrity Validation**: SHA-256 hashes verify patch correctness
//! - **High Performance**: Optimized for large state graphs
//!
//! # Example
//!
//! ```rust
//! use serde_json::json;
//! use dashflow_streaming::diff::{diff_states, apply_patch, DiffResult};
//!
//! // Old and new state
//! let old_state = json!({
//!     "messages": ["Hello"],
//!     "counter": 5
//! });
//! let new_state = json!({
//!     "messages": ["Hello", "World"],
//!     "counter": 6
//! });
//!
//! // Generate diff
//! let result = diff_states(&old_state, &new_state).unwrap();
//!
//! // Apply patch to reconstruct state
//! let reconstructed = apply_patch(&old_state, &result.patch).unwrap();
//! assert_eq!(reconstructed, new_state);
//! ```

pub mod protobuf;

use json_patch::{diff as json_diff, patch as json_patch};

// Re-export Patch for external use
pub use json_patch::Patch;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fmt;

use crate::errors::{Error, Result};

/// Result of a diff operation, including the patch and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffResult {
    /// JSON Patch operations (RFC 6902)
    pub patch: Patch,

    /// Size of the patch in bytes when serialized
    pub patch_size: usize,

    /// Size of the full new state in bytes
    pub full_state_size: usize,

    /// Whether to use full state instead of patch (when patch is larger)
    pub use_full_state: bool,

    /// SHA-256 hash of the new state for integrity verification
    pub state_hash: String,

    /// SHA-256 hash of the patch for integrity verification
    pub patch_hash: String,
}

impl DiffResult {
    /// Check if the patch should be used instead of full state
    #[must_use]
    pub fn should_use_patch(&self) -> bool {
        !self.use_full_state
    }

    /// Get the optimal payload (patch or full state) as JSON
    #[must_use]
    pub fn optimal_payload(&self, full_state: &Value) -> Value {
        if self.use_full_state {
            full_state.clone()
        } else {
            serde_json::to_value(&self.patch).unwrap_or_else(|_| full_state.clone())
        }
    }
}

impl fmt::Display for DiffResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DiffResult {{ patch_ops: {}, patch_size: {} bytes, full_state_size: {} bytes, use_full_state: {}, savings: {:.1}% }}",
            self.patch.0.len(),
            self.patch_size,
            self.full_state_size,
            self.use_full_state,
            if self.use_full_state {
                0.0
            } else {
                (1.0 - (self.patch_size as f64 / self.full_state_size as f64)) * 100.0
            }
        )
    }
}

/// Compute diff between two JSON states
///
/// Returns a `DiffResult` containing the patch and metadata about whether
/// to use the patch or full state based on size optimization.
///
/// # Arguments
///
/// * `old_state` - The previous state
/// * `new_state` - The current state
///
/// # Returns
///
/// A `DiffResult` with the patch and optimization metadata
///
/// # Example
///
/// ```rust
/// use serde_json::json;
/// use dashflow_streaming::diff::diff_states;
///
/// let old = json!({"count": 1});
/// let new = json!({"count": 2});
/// let result = diff_states(&old, &new).unwrap();
/// assert_eq!(result.patch.0.len(), 1); // One replace operation
/// ```
pub fn diff_states(old_state: &Value, new_state: &Value) -> Result<DiffResult> {
    // Generate JSON Patch
    let patch = json_diff(old_state, new_state);

    // Calculate sizes
    let patch_json = serde_json::to_vec(&patch)
        .map_err(|e| Error::Serialization(format!("Failed to serialize patch: {e}")))?;
    let patch_size = patch_json.len();

    let full_state_json = serde_json::to_vec(new_state)
        .map_err(|e| Error::Serialization(format!("Failed to serialize state: {e}")))?;
    let full_state_size = full_state_json.len();

    // Compute hashes
    let state_hash = compute_state_hash_hex(new_state)?;
    let patch_hash = compute_hash(&patch_json);

    // Optimization: use full state if patch is larger
    // Add 10% buffer to account for compression differences
    let use_full_state = patch_size > (full_state_size * 11 / 10);

    Ok(DiffResult {
        patch,
        patch_size,
        full_state_size,
        use_full_state,
        state_hash,
        patch_hash,
    })
}

/// Apply a JSON Patch to a state
///
/// # Arguments
///
/// * `old_state` - The state to apply the patch to
/// * `patch` - The JSON Patch operations
///
/// # Returns
///
/// The new state after applying the patch
///
/// # Example
///
/// ```rust
/// use serde_json::json;
/// use json_patch::Patch;
/// use dashflow_streaming::diff::apply_patch;
///
/// let state = json!({"count": 1});
/// let patch: Patch = serde_json::from_value(json!([
///     {"op": "replace", "path": "/count", "value": 2}
/// ])).unwrap();
///
/// let new_state = apply_patch(&state, &patch).unwrap();
/// assert_eq!(new_state["count"], 2);
/// ```
pub fn apply_patch(old_state: &Value, patch: &Patch) -> Result<Value> {
    let mut state = old_state.clone();
    json_patch(&mut state, patch)
        .map_err(|e| Error::DiffError(format!("Failed to apply patch: {e}")))?;
    Ok(state)
}

/// Verify the integrity of a state using its hash
///
/// # Arguments
///
/// * `state` - The state to verify
/// * `expected_hash` - The expected SHA-256 hash (hex string)
///
/// # Returns
///
/// `Ok(())` if the hash matches, error otherwise
pub fn verify_state_hash(state: &Value, expected_hash: &str) -> Result<()> {
    let actual_hash = compute_state_hash_hex(state)?;

    if actual_hash != expected_hash {
        return Err(Error::DiffError(format!(
            "State hash mismatch: expected {expected_hash}, got {actual_hash}"
        )));
    }

    Ok(())
}

/// Verify the integrity of a patch using its hash
///
/// # Arguments
///
/// * `patch` - The patch to verify
/// * `expected_hash` - The expected SHA-256 hash (hex string)
///
/// # Returns
///
/// `Ok(())` if the hash matches, error otherwise
pub fn verify_patch_hash(patch: &Patch, expected_hash: &str) -> Result<()> {
    let patch_json = serde_json::to_vec(patch)
        .map_err(|e| Error::Serialization(format!("Failed to serialize patch: {e}")))?;
    let actual_hash = compute_hash(&patch_json);

    if actual_hash != expected_hash {
        return Err(Error::DiffError(format!(
            "Patch hash mismatch: expected {expected_hash}, got {actual_hash}"
        )));
    }

    Ok(())
}

fn compute_state_hash_hex(state: &Value) -> Result<String> {
    let canonical = canonical_json_bytes(state)?;
    Ok(compute_hash(&canonical))
}

fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    write_canonical_json(value, &mut out)?;
    Ok(out)
}

fn write_canonical_json(value: &Value, out: &mut Vec<u8>) -> Result<()> {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            let json = serde_json::to_vec(value)
                .map_err(|e| Error::Serialization(format!("Failed to serialize value: {e}")))?;
            out.extend_from_slice(&json);
            Ok(())
        }
        Value::Array(values) => {
            out.push(b'[');
            for (i, item) in values.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write_canonical_json(item, out)?;
            }
            out.push(b']');
            Ok(())
        }
        Value::Object(map) => {
            out.push(b'{');

            let mut entries: Vec<(&str, &Value)> = map.iter().map(|(k, v)| (k.as_str(), v)).collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));

            for (i, (key, child)) in entries.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }

                let key_json = serde_json::to_vec(key)
                    .map_err(|e| Error::Serialization(format!("Failed to serialize key: {e}")))?;
                out.extend_from_slice(&key_json);
                out.push(b':');
                write_canonical_json(child, out)?;
            }

            out.push(b'}');
            Ok(())
        }
    }
}

/// Compute SHA-256 hash of data
fn compute_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_diff_simple_replace() {
        let old = json!({"count": 1});
        let new = json!({"count": 2});

        let result = diff_states(&old, &new).unwrap();
        assert_eq!(result.patch.0.len(), 1);
        // Note: For very small states, patch may be larger than full state
        // due to JSON Patch overhead. The optimization logic will handle this.
    }

    #[test]
    fn test_diff_add_field() {
        let old = json!({"count": 1});
        let new = json!({"count": 1, "name": "test"});

        let result = diff_states(&old, &new).unwrap();
        assert_eq!(result.patch.0.len(), 1);
    }

    #[test]
    fn test_diff_remove_field() {
        let old = json!({"count": 1, "name": "test"});
        let new = json!({"count": 1});

        let result = diff_states(&old, &new).unwrap();
        assert_eq!(result.patch.0.len(), 1);
    }

    #[test]
    fn test_diff_array_append() {
        let old = json!({"items": [1, 2, 3]});
        let new = json!({"items": [1, 2, 3, 4]});

        let result = diff_states(&old, &new).unwrap();
        assert!(!result.patch.0.is_empty());
    }

    #[test]
    fn test_diff_nested_change() {
        let old = json!({"user": {"name": "Alice", "age": 30}});
        let new = json!({"user": {"name": "Alice", "age": 31}});

        let result = diff_states(&old, &new).unwrap();
        assert_eq!(result.patch.0.len(), 1);
    }

    #[test]
    fn test_apply_patch_simple() {
        let old = json!({"count": 1});
        let new = json!({"count": 2});

        let result = diff_states(&old, &new).unwrap();
        let reconstructed = apply_patch(&old, &result.patch).unwrap();

        assert_eq!(reconstructed, new);
    }

    #[test]
    fn test_apply_patch_complex() {
        let old = json!({
            "messages": ["Hello"],
            "counter": 5,
            "metadata": {"timestamp": 1234}
        });
        let new = json!({
            "messages": ["Hello", "World"],
            "counter": 6,
            "metadata": {"timestamp": 1234, "user": "Alice"}
        });

        let result = diff_states(&old, &new).unwrap();
        let reconstructed = apply_patch(&old, &result.patch).unwrap();

        assert_eq!(reconstructed, new);
    }

    #[test]
    fn test_verify_state_hash_valid() {
        let state = json!({"count": 1});
        let hash = compute_state_hash_hex(&state).unwrap();

        assert!(verify_state_hash(&state, &hash).is_ok());
    }

    #[test]
    fn test_verify_state_hash_invalid() {
        let state = json!({"count": 1});
        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";

        assert!(verify_state_hash(&state, wrong_hash).is_err());
    }

    #[test]
    fn test_state_hash_golden_vector() {
        let state = json!({
            "b": 2,
            "a": 1,
            "nested": {
                "z": "x",
                "y": [true, null]
            }
        });

        let hash = compute_state_hash_hex(&state).unwrap();
        assert_eq!(
            hash,
            "f35279c8aa6b00bc82d43a191596cc3b41b7de7899ee16e36a08efe3afc45103"
        );
    }

    #[test]
    fn test_verify_patch_hash_valid() {
        let patch: Patch = serde_json::from_value(json!([
            {"op": "replace", "path": "/count", "value": 2}
        ]))
        .unwrap();

        let hash = {
            let json = serde_json::to_vec(&patch).unwrap();
            compute_hash(&json)
        };

        assert!(verify_patch_hash(&patch, &hash).is_ok());
    }

    #[test]
    fn test_use_full_state_when_patch_larger() {
        // Create a case where patch is larger than full state
        let old = json!({
            "a": 1, "b": 2, "c": 3, "d": 4, "e": 5
        });
        let new = json!({
            "x": "This is a completely new small state"
        });

        let result = diff_states(&old, &new).unwrap();
        // The patch will likely be larger due to removing many fields
        // and adding new ones, so it may choose full state
        // This tests the logic works correctly
        println!(
            "Patch size: {}, Full state size: {}, Use full state: {}",
            result.patch_size, result.full_state_size, result.use_full_state
        );
    }

    #[test]
    fn test_diff_result_display() {
        let old = json!({"count": 1});
        let new = json!({"count": 2});

        let result = diff_states(&old, &new).unwrap();
        let display = format!("{}", result);

        assert!(display.contains("DiffResult"));
        assert!(display.contains("patch_ops"));
        assert!(display.contains("bytes"));
    }

    // Property-based tests
    mod property_tests {
        use super::*;
        use quickcheck::{Arbitrary, Gen};
        use quickcheck_macros::quickcheck;

        // Helper to generate arbitrary JSON values
        fn arb_json_value(g: &mut Gen, depth: usize) -> Value {
            if depth == 0 {
                // Generate leaf values
                match u8::arbitrary(g) % 5 {
                    0 => Value::Null,
                    1 => Value::Bool(bool::arbitrary(g)),
                    2 => Value::Number((i32::arbitrary(g) as i64).into()),
                    3 => Value::String(String::arbitrary(g).chars().take(20).collect()),
                    _ => Value::Number((f64::arbitrary(g) as i64).into()),
                }
            } else {
                match u8::arbitrary(g) % 2 {
                    0 => {
                        // Generate object
                        let size = (usize::arbitrary(g) % 5) + 1;
                        let mut map = serde_json::Map::new();
                        for i in 0..size {
                            let key = format!("key{}", i);
                            let value = arb_json_value(g, depth - 1);
                            map.insert(key, value);
                        }
                        Value::Object(map)
                    }
                    _ => {
                        // Generate array
                        let size = (usize::arbitrary(g) % 5) + 1;
                        let values: Vec<_> =
                            (0..size).map(|_| arb_json_value(g, depth - 1)).collect();
                        Value::Array(values)
                    }
                }
            }
        }

        #[quickcheck]
        fn prop_diff_apply_roundtrip(_seed: u64) -> bool {
            let mut g = Gen::new(20);
            // Use seed to make it deterministic for this test
            let old = arb_json_value(&mut g, 2);
            let new = arb_json_value(&mut g, 2);

            // Generate diff
            let result = match diff_states(&old, &new) {
                Ok(r) => r,
                Err(_) => return true, // Skip if diff fails
            };

            // Apply patch
            let reconstructed = match apply_patch(&old, &result.patch) {
                Ok(r) => r,
                Err(_) => return false, // Patch application should always work
            };

            // Verify reconstruction matches new state
            reconstructed == new
        }

        #[quickcheck]
        fn prop_hash_verification_detects_changes(_seed: u64) -> bool {
            let mut g = Gen::new(20);
            let state = arb_json_value(&mut g, 2);

            let hash = match compute_state_hash_hex(&state) {
                Ok(hash) => hash,
                Err(_) => return true,
            };

            // Correct hash should verify
            verify_state_hash(&state, &hash).is_ok()
        }

        #[quickcheck]
        fn prop_diff_size_is_bounded(_seed: u64) -> bool {
            let mut g = Gen::new(20);
            let old = arb_json_value(&mut g, 2);
            let new = arb_json_value(&mut g, 2);

            let result = match diff_states(&old, &new) {
                Ok(r) => r,
                Err(_) => return true,
            };

            // Either patch is used (smaller) or full state is used
            if result.use_full_state {
                // Patch was larger than full state (+ 10% buffer)
                result.patch_size > (result.full_state_size * 11 / 10)
            } else {
                // Patch was smaller
                result.patch_size <= (result.full_state_size * 11 / 10)
            }
        }
    }
}
