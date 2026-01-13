// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Protobuf integration for state diff operations
//!
//! This module provides conversion between json-patch Patch objects
//! and DashFlow Streaming protobuf `DiffOperation` messages.
//!
//! # Value Encoding Policy
//!
//! **Browser clients MUST receive JSON or RAW encoded values.** The observability UI
//! cannot decode MSGPACK or PROTOBUF encodings as they require additional libraries
//! and/or schema definitions not available in the browser environment.
//!
//! The producer functions in this module always emit `ValueEncoding::Json` to ensure
//! browser compatibility. If you're extending the streaming system:
//!
//! - **For browser-facing telemetry**: Use `ValueEncoding::Json` (default)
//! - **For server-to-server communication**: MSGPACK/PROTOBUF may be used for efficiency
//!
//! When the consumer receives non-JSON encoded values intended for browser clients,
//! diagnostics are emitted via the `dashstream_unsupported_encoding_total` metric
//! and a warning is logged. Configure your producer to use JSON encoding.
//!
//! See `observability-ui/src/utils/jsonPatch.ts` for the UI-side handling.

use json_patch::{Patch, PatchOperation};
use prometheus::Counter;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::LazyLock;
use tracing::warn;

use crate::errors::{Error, Result};
use crate::{
    diff_operation::{OpType, ValueEncoding},
    DiffOperation, Header, StateDiff,
};

// ============================================================================
// Error Helpers (CQ-9: Reduce repetitive error mapping)
// ============================================================================

/// Create a serialization error for value serialization failures.
#[inline]
fn ser_err(e: impl std::fmt::Display) -> Error {
    Error::Serialization(format!("Failed to serialize value: {e}"))
}

/// Create a diff error for value deserialization failures.
#[inline]
fn deser_err(e: impl std::fmt::Display) -> Error {
    Error::DiffError(format!("Failed to deserialize value: {e}"))
}

/// Create a diff error for invalid JSON path.
#[inline]
fn path_err(e: impl std::fmt::Display) -> Error {
    Error::DiffError(format!("Invalid path: {e}"))
}

/// Create a diff error for invalid "from" path.
#[inline]
fn from_path_err(e: impl std::fmt::Display) -> Error {
    Error::DiffError(format!("Invalid from path: {e}"))
}

// Diagnostic counter for unsupported encodings (M-94: encoding policy enforcement)
// M-624: Use centralized constants
use crate::metrics_constants::METRIC_UNSUPPORTED_ENCODING_TOTAL;

static UNSUPPORTED_ENCODING_TOTAL: LazyLock<Counter> = LazyLock::new(|| {
    crate::metrics_utils::counter(
        METRIC_UNSUPPORTED_ENCODING_TOTAL,
        "Total number of StateDiff operations received with unsupported encodings (MSGPACK/PROTOBUF)",
    )
});

/// Check encoding and emit diagnostics for browser-incompatible encodings.
///
/// Returns Ok(()) for JSON/RAW, Err for MSGPACK/PROTOBUF with diagnostics emitted.
fn check_encoding_for_browser(encoding: i32, path: &str) -> Result<()> {
    match ValueEncoding::try_from(encoding) {
        Ok(ValueEncoding::Json) | Ok(ValueEncoding::Raw) => Ok(()),
        Ok(ValueEncoding::Msgpack) => {
            UNSUPPORTED_ENCODING_TOTAL.inc();
            warn!(
                encoding = "MSGPACK",
                path = %path,
                "StateDiff operation uses MSGPACK encoding which is unsupported by browser clients. \
                 Configure your producer to use JSON encoding for browser-facing telemetry."
            );
            Err(Error::DiffError(format!(
                "MSGPACK encoding not supported for browser clients at path '{path}'. Use JSON encoding."
            )))
        }
        Ok(ValueEncoding::Protobuf) => {
            UNSUPPORTED_ENCODING_TOTAL.inc();
            warn!(
                encoding = "PROTOBUF",
                path = %path,
                "StateDiff operation uses PROTOBUF encoding which is unsupported by browser clients. \
                 Configure your producer to use JSON encoding for browser-facing telemetry."
            );
            Err(Error::DiffError(format!(
                "PROTOBUF encoding not supported for browser clients at path '{path}'. Use JSON encoding."
            )))
        }
        Err(_) => {
            warn!(
                encoding = encoding,
                path = %path,
                "Unknown encoding type in StateDiff operation"
            );
            // Allow unknown encodings to proceed (might be JSON-compatible)
            Ok(())
        }
    }
}

/// Convert a json-patch Patch to protobuf `DiffOperations`
pub fn patch_to_proto(patch: &Patch) -> Result<Vec<DiffOperation>> {
    patch.0.iter().map(operation_to_proto).collect()
}

/// Convert a single json-patch `PatchOperation` to protobuf `DiffOperation`
fn operation_to_proto(op: &PatchOperation) -> Result<DiffOperation> {
    match op {
        PatchOperation::Add(add_op) => {
            let value_json = serde_json::to_vec(&add_op.value).map_err(ser_err)?;

            Ok(DiffOperation {
                op: OpType::Add as i32,
                path: add_op.path.to_string(),
                value: value_json,
                from: String::new(),
                encoding: ValueEncoding::Json as i32,
            })
        }
        PatchOperation::Remove(remove_op) => Ok(DiffOperation {
            op: OpType::Remove as i32,
            path: remove_op.path.to_string(),
            value: vec![],
            from: String::new(),
            encoding: ValueEncoding::Json as i32,
        }),
        PatchOperation::Replace(replace_op) => {
            let value_json = serde_json::to_vec(&replace_op.value).map_err(ser_err)?;

            Ok(DiffOperation {
                op: OpType::Replace as i32,
                path: replace_op.path.to_string(),
                value: value_json,
                from: String::new(),
                encoding: ValueEncoding::Json as i32,
            })
        }
        PatchOperation::Move(move_op) => Ok(DiffOperation {
            op: OpType::Move as i32,
            path: move_op.path.to_string(),
            value: vec![],
            from: move_op.from.to_string(),
            encoding: ValueEncoding::Json as i32,
        }),
        PatchOperation::Copy(copy_op) => Ok(DiffOperation {
            op: OpType::Copy as i32,
            path: copy_op.path.to_string(),
            value: vec![],
            from: copy_op.from.to_string(),
            encoding: ValueEncoding::Json as i32,
        }),
        PatchOperation::Test(test_op) => {
            let value_json = serde_json::to_vec(&test_op.value).map_err(ser_err)?;

            Ok(DiffOperation {
                op: OpType::Test as i32,
                path: test_op.path.to_string(),
                value: value_json,
                from: String::new(),
                encoding: ValueEncoding::Json as i32,
            })
        }
    }
}

/// Convert protobuf `DiffOperations` to a json-patch Patch
pub fn proto_to_patch(operations: &[DiffOperation]) -> Result<Patch> {
    let patch_ops: Result<Vec<PatchOperation>> =
        operations.iter().map(proto_to_operation).collect();

    Ok(Patch(patch_ops?))
}

/// Convert a protobuf `DiffOperation` to a json-patch `PatchOperation`
///
/// # Errors
///
/// Returns an error if:
/// - The operation type is invalid
/// - The encoding is MSGPACK or PROTOBUF (not supported for browser clients)
/// - The JSON path is malformed
/// - The value cannot be deserialized
fn proto_to_operation(op: &DiffOperation) -> Result<PatchOperation> {
    // M-94: Enforce encoding policy - check before processing
    check_encoding_for_browser(op.encoding, &op.path)?;

    let op_type = OpType::try_from(op.op)
        .map_err(|e| Error::DiffError(format!("Invalid operation type {}: {e:?}", op.op)))?;

    match op_type {
        OpType::Add => {
            let value: Value = if op.value.is_empty() {
                Value::Null
            } else {
                serde_json::from_slice(&op.value).map_err(deser_err)?
            };

            Ok(PatchOperation::Add(json_patch::AddOperation {
                path: op.path.parse().map_err(path_err)?,
                value,
            }))
        }
        OpType::Remove => Ok(PatchOperation::Remove(json_patch::RemoveOperation {
            path: op.path.parse().map_err(path_err)?,
        })),
        OpType::Replace => {
            let value: Value = if op.value.is_empty() {
                Value::Null
            } else {
                serde_json::from_slice(&op.value).map_err(deser_err)?
            };

            Ok(PatchOperation::Replace(json_patch::ReplaceOperation {
                path: op.path.parse().map_err(path_err)?,
                value,
            }))
        }
        OpType::Move => Ok(PatchOperation::Move(json_patch::MoveOperation {
            path: op.path.parse().map_err(path_err)?,
            from: op.from.parse().map_err(from_path_err)?,
        })),
        OpType::Copy => Ok(PatchOperation::Copy(json_patch::CopyOperation {
            path: op.path.parse().map_err(path_err)?,
            from: op.from.parse().map_err(from_path_err)?,
        })),
        OpType::Test => {
            let value: Value = if op.value.is_empty() {
                Value::Null
            } else {
                serde_json::from_slice(&op.value).map_err(deser_err)?
            };

            Ok(PatchOperation::Test(json_patch::TestOperation {
                path: op.path.parse().map_err(path_err)?,
                value,
            }))
        }
    }
}

/// Create a `StateDiff` message from old and new states
///
/// This is a convenience function that combines `diff_states()` with protobuf conversion
pub fn create_state_diff(
    header: Header,
    base_checkpoint_id: Vec<u8>,
    old_state: &Value,
    new_state: &Value,
) -> Result<StateDiff> {
    let diff_result = crate::diff::diff_states(old_state, new_state)?;

    // Convert hash string to bytes
    let state_hash = hex::decode(&diff_result.state_hash)
        .map_err(|e| Error::DiffError(format!("Failed to decode hash: {e}")))?;

    // Check if we should use full state
    let (operations, full_state) = if diff_result.use_full_state {
        // Use full state instead of diff
        let full_state_json = serde_json::to_vec(new_state)
            .map_err(|e| Error::Serialization(format!("Failed to serialize state: {e}")))?;
        (vec![], full_state_json)
    } else {
        // Use diff operations
        let operations = patch_to_proto(&diff_result.patch)?;
        (operations, vec![])
    };

    Ok(StateDiff {
        header: Some(header),
        base_checkpoint_id,
        operations,
        state_hash,
        full_state,
    })
}

/// Apply a `StateDiff` to a base state
pub fn apply_state_diff(base_state: &Value, state_diff: &StateDiff) -> Result<Value> {
    // Reconstruct state from full snapshot or patch.
    let new_state = if !state_diff.full_state.is_empty() {
        serde_json::from_slice(&state_diff.full_state)
            .map_err(|e| Error::DiffError(format!("Failed to deserialize full state: {e}")))?
    } else {
        let patch = proto_to_patch(&state_diff.operations)?;
        crate::diff::apply_patch(base_state, &patch)?
    };

    // Verify integrity if a state hash is provided (legacy diffs may omit this).
    if !state_diff.state_hash.is_empty() {
        let state_json = super::canonical_json_bytes(&new_state)?;
        let mut hasher = Sha256::new();
        hasher.update(&state_json);
        let actual_hash = hasher.finalize();

        if actual_hash.as_slice() != state_diff.state_hash.as_slice() {
            return Err(Error::DiffError(format!(
                "State hash mismatch: expected {}, got {}",
                hex::encode(&state_diff.state_hash),
                hex::encode(actual_hash)
            )));
        }
    }

    Ok(new_state)
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_patch_to_proto_roundtrip() {
        let old = json!({"count": 1});
        let new = json!({"count": 2});

        let diff_result = crate::diff::diff_states(&old, &new).unwrap();
        let proto_ops = patch_to_proto(&diff_result.patch).unwrap();
        let reconstructed_patch = proto_to_patch(&proto_ops).unwrap();

        let result = crate::diff::apply_patch(&old, &reconstructed_patch).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_create_state_diff() {
        let old = json!({
            "messages": ["Hello"],
            "counter": 5
        });
        let new = json!({
            "messages": ["Hello", "World"],
            "counter": 6
        });

        let header = Header {
            message_id: vec![1, 2, 3, 4],
            timestamp_us: 123456,
            tenant_id: "test".to_string(),
            thread_id: "thread1".to_string(),
            sequence: 1,
            r#type: crate::MessageType::StateDiff as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: 1,
        };

        let state_diff = create_state_diff(header, vec![], &old, &new).unwrap();

        assert!(state_diff.header.is_some());
        assert!(!state_diff.state_hash.is_empty());
    }

    #[test]
    fn test_apply_state_diff_with_operations() {
        let old = json!({"count": 1});
        let new = json!({"count": 2});

        let header = Header {
            message_id: vec![1, 2, 3, 4],
            timestamp_us: 123456,
            tenant_id: "test".to_string(),
            thread_id: "thread1".to_string(),
            sequence: 1,
            r#type: crate::MessageType::StateDiff as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: 1,
        };

        let state_diff = create_state_diff(header, vec![], &old, &new).unwrap();

        let result = apply_state_diff(&old, &state_diff).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_apply_state_diff_with_full_state() {
        // Create a case where full state is used
        let old = json!({
            "a": 1, "b": 2, "c": 3, "d": 4, "e": 5
        });
        let new = json!({
            "x": "small"
        });

        let header = Header {
            message_id: vec![1, 2, 3, 4],
            timestamp_us: 123456,
            tenant_id: "test".to_string(),
            thread_id: "thread1".to_string(),
            sequence: 1,
            r#type: crate::MessageType::StateDiff as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: 1,
        };

        let state_diff = create_state_diff(header, vec![], &old, &new).unwrap();

        let result = apply_state_diff(&old, &state_diff).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_operation_to_proto_add() {
        use json_patch::AddOperation;

        let add_op = PatchOperation::Add(AddOperation {
            path: "/count".parse().unwrap(),
            value: json!(42),
        });

        let proto_op = operation_to_proto(&add_op).unwrap();
        assert_eq!(proto_op.op, OpType::Add as i32);
        assert_eq!(proto_op.path, "/count");
        assert_eq!(proto_op.encoding, ValueEncoding::Json as i32);
        assert!(!proto_op.value.is_empty());

        // Verify value can be deserialized
        let value: Value = serde_json::from_slice(&proto_op.value).unwrap();
        assert_eq!(value, json!(42));
    }

    #[test]
    fn test_operation_to_proto_remove() {
        use json_patch::RemoveOperation;

        let remove_op = PatchOperation::Remove(RemoveOperation {
            path: "/old_field".parse().unwrap(),
        });

        let proto_op = operation_to_proto(&remove_op).unwrap();
        assert_eq!(proto_op.op, OpType::Remove as i32);
        assert_eq!(proto_op.path, "/old_field");
        assert!(proto_op.value.is_empty());
        assert_eq!(proto_op.from, "");
    }

    #[test]
    fn test_operation_to_proto_replace() {
        use json_patch::ReplaceOperation;

        let replace_op = PatchOperation::Replace(ReplaceOperation {
            path: "/status".parse().unwrap(),
            value: json!("active"),
        });

        let proto_op = operation_to_proto(&replace_op).unwrap();
        assert_eq!(proto_op.op, OpType::Replace as i32);
        assert_eq!(proto_op.path, "/status");
        assert!(!proto_op.value.is_empty());
    }

    #[test]
    fn test_operation_to_proto_move() {
        use json_patch::MoveOperation;

        let move_op = PatchOperation::Move(MoveOperation {
            path: "/destination".parse().unwrap(),
            from: "/source".parse().unwrap(),
        });

        let proto_op = operation_to_proto(&move_op).unwrap();
        assert_eq!(proto_op.op, OpType::Move as i32);
        assert_eq!(proto_op.path, "/destination");
        assert_eq!(proto_op.from, "/source");
        assert!(proto_op.value.is_empty());
    }

    #[test]
    fn test_operation_to_proto_copy() {
        use json_patch::CopyOperation;

        let copy_op = PatchOperation::Copy(CopyOperation {
            path: "/destination".parse().unwrap(),
            from: "/source".parse().unwrap(),
        });

        let proto_op = operation_to_proto(&copy_op).unwrap();
        assert_eq!(proto_op.op, OpType::Copy as i32);
        assert_eq!(proto_op.path, "/destination");
        assert_eq!(proto_op.from, "/source");
    }

    #[test]
    fn test_operation_to_proto_test() {
        use json_patch::TestOperation;

        let test_op = PatchOperation::Test(TestOperation {
            path: "/version".parse().unwrap(),
            value: json!(2),
        });

        let proto_op = operation_to_proto(&test_op).unwrap();
        assert_eq!(proto_op.op, OpType::Test as i32);
        assert_eq!(proto_op.path, "/version");
        assert!(!proto_op.value.is_empty());
    }

    #[test]
    fn test_proto_to_operation_add_with_null_value() {
        let proto_op = DiffOperation {
            op: OpType::Add as i32,
            path: "/field".to_string(),
            value: vec![], // Empty value should become null
            from: String::new(),
            encoding: ValueEncoding::Json as i32,
        };

        let patch_op = proto_to_operation(&proto_op).unwrap();
        match patch_op {
            PatchOperation::Add(add_op) => {
                assert_eq!(add_op.path.to_string(), "/field");
                assert_eq!(add_op.value, Value::Null);
            }
            _ => panic!("Expected Add operation"),
        }
    }

    #[test]
    fn test_proto_to_operation_replace_with_null() {
        let proto_op = DiffOperation {
            op: OpType::Replace as i32,
            path: "/nullable".to_string(),
            value: vec![],
            from: String::new(),
            encoding: ValueEncoding::Json as i32,
        };

        let patch_op = proto_to_operation(&proto_op).unwrap();
        match patch_op {
            PatchOperation::Replace(replace_op) => {
                assert_eq!(replace_op.value, Value::Null);
            }
            _ => panic!("Expected Replace operation"),
        }
    }

    #[test]
    fn test_proto_to_operation_test_with_null() {
        let proto_op = DiffOperation {
            op: OpType::Test as i32,
            path: "/check".to_string(),
            value: vec![],
            from: String::new(),
            encoding: ValueEncoding::Json as i32,
        };

        let patch_op = proto_to_operation(&proto_op).unwrap();
        match patch_op {
            PatchOperation::Test(test_op) => {
                assert_eq!(test_op.value, Value::Null);
            }
            _ => panic!("Expected Test operation"),
        }
    }

    #[test]
    fn test_proto_to_operation_invalid_op_type() {
        let proto_op = DiffOperation {
            op: 999, // Invalid operation type
            path: "/field".to_string(),
            value: vec![],
            from: String::new(),
            encoding: ValueEncoding::Json as i32,
        };

        let result = proto_to_operation(&proto_op);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::DiffError(_)));
    }

    #[test]
    fn test_proto_to_operation_invalid_json() {
        let proto_op = DiffOperation {
            op: OpType::Add as i32,
            path: "/field".to_string(),
            value: vec![0xFF, 0xFF, 0xFF], // Invalid JSON
            from: String::new(),
            encoding: ValueEncoding::Json as i32,
        };

        let result = proto_to_operation(&proto_op);
        assert!(result.is_err());
    }

    #[test]
    fn test_proto_to_operation_invalid_path() {
        let proto_op = DiffOperation {
            op: OpType::Add as i32,
            path: "invalid-path-without-slash".to_string(),
            value: serde_json::to_vec(&json!(42)).unwrap(),
            from: String::new(),
            encoding: ValueEncoding::Json as i32,
        };

        let result = proto_to_operation(&proto_op);
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_state_diff_empty_full_state_and_operations() {
        let base_state = json!({"count": 1});

        let state_diff = StateDiff {
            header: None,
            base_checkpoint_id: vec![],
            operations: vec![], // No operations
            state_hash: vec![],
            full_state: vec![], // No full state
        };

        // With no operations and no full state, should return base state unchanged
        let result = apply_state_diff(&base_state, &state_diff).unwrap();
        assert_eq!(result, base_state);
    }

    #[test]
    fn test_create_state_diff_with_base_checkpoint() {
        let old = json!({"count": 1});
        let new = json!({"count": 2});

        let header = Header {
            message_id: vec![1, 2, 3, 4],
            timestamp_us: 123456,
            tenant_id: "test".to_string(),
            thread_id: "thread1".to_string(),
            sequence: 1,
            r#type: crate::MessageType::StateDiff as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: 1,
        };

        let base_checkpoint_id = vec![0xDE, 0xAD, 0xBE, 0xEF];

        let state_diff = create_state_diff(header, base_checkpoint_id.clone(), &old, &new).unwrap();

        assert_eq!(state_diff.base_checkpoint_id, base_checkpoint_id);
        assert!(state_diff.header.is_some());
    }

    #[test]
    fn test_patch_to_proto_empty_patch() {
        use json_patch::Patch;

        let empty_patch = Patch(vec![]);
        let proto_ops = patch_to_proto(&empty_patch).unwrap();
        assert!(proto_ops.is_empty());
    }

    #[test]
    fn test_proto_to_patch_empty_operations() {
        let empty_ops: Vec<DiffOperation> = vec![];
        let patch = proto_to_patch(&empty_ops).unwrap();
        assert_eq!(patch.0.len(), 0);
    }

    // M-94: Encoding policy enforcement tests

    #[test]
    fn test_encoding_policy_json_allowed() {
        let proto_op = DiffOperation {
            op: OpType::Add as i32,
            path: "/field".to_string(),
            value: serde_json::to_vec(&json!(42)).unwrap(),
            from: String::new(),
            encoding: ValueEncoding::Json as i32,
        };

        let result = proto_to_operation(&proto_op);
        assert!(result.is_ok());
    }

    #[test]
    fn test_encoding_policy_raw_allowed() {
        // RAW encoding is allowed - it's just UTF-8 text
        let proto_op = DiffOperation {
            op: OpType::Add as i32,
            path: "/field".to_string(),
            value: b"hello world".to_vec(),
            from: String::new(),
            encoding: ValueEncoding::Raw as i32,
        };

        // RAW encoding is allowed but value parsing will fail since it's not JSON
        // The encoding check passes, but deserialization fails
        let result = proto_to_operation(&proto_op);
        // This fails at JSON parse, not at encoding check
        assert!(result.is_err());
    }

    #[test]
    fn test_encoding_policy_msgpack_rejected() {
        let proto_op = DiffOperation {
            op: OpType::Add as i32,
            path: "/field".to_string(),
            value: vec![0x92, 0x01, 0x02], // MessagePack array [1, 2]
            from: String::new(),
            encoding: ValueEncoding::Msgpack as i32,
        };

        let result = proto_to_operation(&proto_op);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, Error::DiffError(_)));
        assert!(err.to_string().contains("MSGPACK"));
        assert!(err.to_string().contains("browser"));
    }

    #[test]
    fn test_encoding_policy_protobuf_rejected() {
        let proto_op = DiffOperation {
            op: OpType::Add as i32,
            path: "/field".to_string(),
            value: vec![0x08, 0x01], // Protobuf field 1 = 1
            from: String::new(),
            encoding: ValueEncoding::Protobuf as i32,
        };

        let result = proto_to_operation(&proto_op);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, Error::DiffError(_)));
        assert!(err.to_string().contains("PROTOBUF"));
        assert!(err.to_string().contains("browser"));
    }

    #[test]
    fn test_encoding_policy_unknown_encoding_allowed() {
        // Unknown encodings are allowed (with warning) for forward compatibility
        let proto_op = DiffOperation {
            op: OpType::Add as i32,
            path: "/field".to_string(),
            value: serde_json::to_vec(&json!(42)).unwrap(),
            from: String::new(),
            encoding: 999, // Unknown encoding
        };

        // Unknown encoding passes check (warns but continues) if value is valid JSON
        let result = proto_to_operation(&proto_op);
        assert!(result.is_ok());
    }
}
