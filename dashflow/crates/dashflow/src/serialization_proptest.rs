//! Property-based tests for serialization roundtrips
//!
//! This module contains proptest-based tests that verify serialization/deserialization
//! roundtrips for core DashFlow types. Property-based testing generates random inputs
//! to discover edge cases that hand-written tests might miss.
//!
//! # Tested Invariants
//!
//! For all types tested here, we verify:
//! 1. **JSON Roundtrip**: `deserialize(serialize(x)) == x` (where equality can be tested)
//! 2. **Bincode Roundtrip**: `deserialize(serialize(x)) == x` (where applicable)
//! 3. **No Panics**: Serialization and deserialization don't panic on valid input
//! 4. **JSON Stability**: `serialize(deserialize(serialize(x))) == serialize(x)`
//!
//! # Coverage
//!
//! - Message types: ContentBlock, Message, MessageContent, ToolCall, etc.
//! - Graph registry types: StateDiff, FieldDiff (JSON stability test)
//!
//! # Usage
//!
//! Run these tests with:
//! ```bash
//! cargo test -p dashflow serialization_proptest --release
//! ```
//!
//! For more iterations (to find rarer edge cases):
//! ```bash
//! PROPTEST_CASES=10000 cargo test -p dashflow serialization_proptest --release
//! ```

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    // =========================================================================
    // Strategy Helpers
    // =========================================================================

    /// Generate a valid UTF-8 string that won't cause JSON escaping issues
    fn valid_string() -> impl Strategy<Value = String> {
        // Use printable ASCII + common unicode to avoid control characters
        proptest::string::string_regex("[a-zA-Z0-9 _.,!?@#$%^&*()-+=\\[\\]{}|;:'\"<>/~`]{0,100}")
            .unwrap()
    }

    /// Generate a valid identifier (for IDs, names, etc.)
    fn valid_id() -> impl Strategy<Value = String> {
        proptest::string::string_regex("[a-zA-Z][a-zA-Z0-9_-]{0,50}").unwrap()
    }

    /// Generate a valid URL-like string
    fn valid_url() -> impl Strategy<Value = String> {
        proptest::string::string_regex("https://[a-z]{1,20}\\.[a-z]{2,5}/[a-zA-Z0-9/_-]{0,50}")
            .unwrap()
    }

    /// Generate valid base64 data
    fn valid_base64() -> impl Strategy<Value = String> {
        prop::collection::vec(prop::num::u8::ANY, 0..100).prop_map(|bytes| {
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes)
        })
    }

    /// Generate valid JSON values (excludes Null for Option<Value> roundtrip stability)
    fn valid_json_value() -> impl Strategy<Value = serde_json::Value> {
        prop_oneof![
            // Note: We exclude Null at top level because Option<Value> with Some(Null)
            // has asymmetric serialization (serializes to null, deserializes to Some(Null)).
            // This is intentional serde behavior, not a bug.
            prop::bool::ANY.prop_map(serde_json::Value::Bool),
            prop::num::i64::ANY.prop_map(|n| serde_json::Value::Number(n.into())),
            valid_string().prop_map(serde_json::Value::String),
            // Arrays and objects (limited depth to prevent stack overflow)
            prop::collection::vec(
                prop_oneof![
                    Just(serde_json::Value::Null), // Null in arrays is fine
                    prop::bool::ANY.prop_map(serde_json::Value::Bool),
                    prop::num::i64::ANY.prop_map(|n| serde_json::Value::Number(n.into())),
                    valid_string().prop_map(serde_json::Value::String),
                ],
                0..5
            )
            .prop_map(serde_json::Value::Array),
        ]
    }

    /// Generate JSON values that work for Option<Value> fields (no Null)
    fn valid_json_value_non_null() -> impl Strategy<Value = serde_json::Value> {
        prop_oneof![
            prop::bool::ANY.prop_map(serde_json::Value::Bool),
            prop::num::i64::ANY.prop_map(|n| serde_json::Value::Number(n.into())),
            valid_string().prop_map(serde_json::Value::String),
        ]
    }

    /// Helper to test JSON roundtrip for types with PartialEq
    fn json_roundtrip<T>(value: &T) -> Result<T, String>
    where
        T: Serialize + for<'de> Deserialize<'de>,
    {
        let json = serde_json::to_string(value).map_err(|e| format!("serialize failed: {e}"))?;
        serde_json::from_str(&json).map_err(|e| format!("deserialize failed: {e}"))
    }

    /// Helper to test JSON stability: serialize -> deserialize -> serialize produces same JSON
    /// This is useful for types without PartialEq
    fn json_stable<T>(value: &T) -> Result<(), String>
    where
        T: Serialize + for<'de> Deserialize<'de>,
    {
        let json1 = serde_json::to_string(value).map_err(|e| format!("serialize failed: {e}"))?;
        let deserialized: T =
            serde_json::from_str(&json1).map_err(|e| format!("deserialize failed: {e}"))?;
        let json2 = serde_json::to_string(&deserialized)
            .map_err(|e| format!("re-serialize failed: {e}"))?;
        if json1 != json2 {
            Err(format!(
                "JSON not stable:\nFirst:  {json1}\nSecond: {json2}"
            ))
        } else {
            Ok(())
        }
    }

    // Note: bincode_stable() helper removed - serde_json::Value doesn't serialize
    // reliably with bincode. JSON is the primary serialization format for tested types.

    // =========================================================================
    // Message Type Strategies
    // =========================================================================

    use crate::core::messages::{
        BaseMessageFields, ContentBlock, ImageDetail, ImageSource, InvalidToolCall, Message,
        MessageContent, ToolCall,
    };
    use crate::core::usage::UsageMetadata;

    fn arb_image_detail() -> impl Strategy<Value = ImageDetail> {
        prop_oneof![
            Just(ImageDetail::Low),
            Just(ImageDetail::High),
            Just(ImageDetail::Auto),
        ]
    }

    fn arb_image_source() -> impl Strategy<Value = ImageSource> {
        prop_oneof![
            valid_url().prop_map(|url| ImageSource::Url { url }),
            (valid_string(), valid_base64())
                .prop_map(|(media_type, data)| ImageSource::Base64 { media_type, data }),
        ]
    }

    fn arb_content_block() -> impl Strategy<Value = ContentBlock> {
        prop_oneof![
            // Text variant
            valid_string().prop_map(|text| ContentBlock::Text { text }),
            // Image variant
            (arb_image_source(), proptest::option::of(arb_image_detail()))
                .prop_map(|(source, detail)| ContentBlock::Image { source, detail }),
            // ToolUse variant
            (valid_id(), valid_id(), valid_json_value())
                .prop_map(|(id, name, input)| { ContentBlock::ToolUse { id, name, input } }),
            // ToolResult variant
            (valid_id(), valid_string(), prop::bool::ANY).prop_map(
                |(tool_use_id, content, is_error)| ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                }
            ),
            // Reasoning variant
            valid_string().prop_map(|reasoning| ContentBlock::Reasoning { reasoning }),
            // Thinking variant
            (valid_string(), proptest::option::of(valid_string())).prop_map(
                |(thinking, signature)| ContentBlock::Thinking {
                    thinking,
                    signature
                }
            ),
            // RedactedThinking variant
            valid_string().prop_map(|data| ContentBlock::RedactedThinking { data }),
        ]
    }

    fn arb_message_content() -> impl Strategy<Value = MessageContent> {
        prop_oneof![
            valid_string().prop_map(MessageContent::Text),
            prop::collection::vec(arb_content_block(), 0..5).prop_map(MessageContent::Blocks),
        ]
    }

    fn arb_tool_call() -> impl Strategy<Value = ToolCall> {
        (
            valid_id(),
            valid_id(),
            valid_json_value(),
            proptest::option::of(0usize..100),
        )
            .prop_map(|(id, name, args, index)| ToolCall {
                id,
                name,
                args,
                tool_type: "tool_call".to_string(),
                index,
            })
    }

    fn arb_invalid_tool_call() -> impl Strategy<Value = InvalidToolCall> {
        (
            valid_id(),
            proptest::option::of(valid_id()),
            proptest::option::of(valid_string()),
            valid_string(),
        )
            .prop_map(|(id, name, args, error)| InvalidToolCall {
                id,
                name,
                args,
                error,
            })
    }

    fn arb_base_message_fields() -> impl Strategy<Value = BaseMessageFields> {
        (
            proptest::option::of(valid_id()),
            proptest::option::of(valid_string()),
        )
            .prop_map(|(id, name)| BaseMessageFields {
                id,
                name,
                additional_kwargs: HashMap::new(),
                response_metadata: HashMap::new(),
            })
    }

    fn arb_usage_metadata() -> impl Strategy<Value = UsageMetadata> {
        (0u32..10000, 0u32..10000, 0u32..20000).prop_map(
            |(input_tokens, output_tokens, total_tokens)| UsageMetadata {
                input_tokens,
                output_tokens,
                total_tokens,
                ..Default::default()
            },
        )
    }

    fn arb_message() -> impl Strategy<Value = Message> {
        prop_oneof![
            // Human message
            (arb_message_content(), arb_base_message_fields())
                .prop_map(|(content, fields)| { Message::Human { content, fields } }),
            // AI message
            (
                arb_message_content(),
                prop::collection::vec(arb_tool_call(), 0..3),
                prop::collection::vec(arb_invalid_tool_call(), 0..2),
                proptest::option::of(arb_usage_metadata()),
                arb_base_message_fields(),
            )
                .prop_map(
                    |(content, tool_calls, invalid_tool_calls, usage_metadata, fields)| {
                        Message::AI {
                            content,
                            tool_calls,
                            invalid_tool_calls,
                            usage_metadata,
                            fields,
                        }
                    }
                ),
            // System message
            (arb_message_content(), arb_base_message_fields())
                .prop_map(|(content, fields)| { Message::System { content, fields } }),
            // Tool message
            // Note: artifact uses non_null JSON values because Option<Value> with Some(Null)
            // has asymmetric serialization in serde (null deserializes to Some(Null), not None)
            (
                arb_message_content(),
                valid_id(),
                proptest::option::of(valid_json_value_non_null()),
                proptest::option::of(valid_string()),
                arb_base_message_fields(),
            )
                .prop_map(|(content, tool_call_id, artifact, status, fields)| {
                    Message::Tool {
                        content,
                        tool_call_id,
                        artifact,
                        status,
                        fields,
                    }
                }),
        ]
    }

    // =========================================================================
    // Checkpoint Type Strategies
    // =========================================================================

    use crate::checkpoint::CheckpointIntegrityError;

    fn arb_checkpoint_integrity_error() -> impl Strategy<Value = CheckpointIntegrityError> {
        prop_oneof![
            (0usize..100, 20usize..200).prop_map(|(size, minimum)| {
                CheckpointIntegrityError::FileTooSmall { size, minimum }
            }),
            prop::array::uniform4(prop::num::u8::ANY).prop_map(|found| {
                CheckpointIntegrityError::InvalidMagic {
                    expected: *b"DCHK",
                    found,
                }
            }),
            (2u32..100, Just(1u32)).prop_map(|(found, supported)| {
                CheckpointIntegrityError::UnsupportedVersion { found, supported }
            }),
            (prop::num::u32::ANY, prop::num::u32::ANY)
                .prop_filter("expected != computed", |(e, c)| e != c)
                .prop_map(
                    |(expected, computed)| CheckpointIntegrityError::ChecksumMismatch {
                        expected,
                        computed,
                    }
                ),
            (prop::num::u64::ANY, prop::num::u64::ANY)
                .prop_filter("declared != actual", |(d, a)| d != a)
                .prop_map(
                    |(declared, actual)| CheckpointIntegrityError::LengthMismatch {
                        declared,
                        actual,
                    }
                ),
        ]
    }

    // =========================================================================
    // Graph Registry Type Strategies
    // =========================================================================

    use crate::graph_registry::{FieldDiff, StateDiff};

    fn arb_field_diff() -> impl Strategy<Value = FieldDiff> {
        (valid_string(), valid_json_value(), valid_json_value()).prop_map(
            |(path, before, after)| FieldDiff {
                path,
                before,
                after,
            },
        )
    }

    fn arb_state_diff() -> impl Strategy<Value = StateDiff> {
        (
            prop::collection::vec(valid_string(), 0..5),
            prop::collection::vec(valid_string(), 0..5),
            prop::collection::vec(arb_field_diff(), 0..5),
        )
            .prop_map(|(added, removed, modified)| StateDiff {
                added,
                removed,
                modified,
            })
    }

    // =========================================================================
    // Property Tests: Message Types (with PartialEq)
    // =========================================================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(500))]

        #[test]
        fn content_block_json_roundtrip(block in arb_content_block()) {
            let result = json_roundtrip(&block);
            prop_assert!(result.is_ok(), "JSON roundtrip failed: {:?}", result.err());
            prop_assert_eq!(result.unwrap(), block);
        }

        #[test]
        fn message_content_json_roundtrip(content in arb_message_content()) {
            let result = json_roundtrip(&content);
            prop_assert!(result.is_ok(), "JSON roundtrip failed: {:?}", result.err());
            prop_assert_eq!(result.unwrap(), content);
        }

        #[test]
        fn tool_call_json_roundtrip(call in arb_tool_call()) {
            let result = json_roundtrip(&call);
            prop_assert!(result.is_ok(), "JSON roundtrip failed: {:?}", result.err());
            prop_assert_eq!(result.unwrap(), call);
        }

        #[test]
        fn invalid_tool_call_json_roundtrip(call in arb_invalid_tool_call()) {
            let result = json_roundtrip(&call);
            prop_assert!(result.is_ok(), "JSON roundtrip failed: {:?}", result.err());
            prop_assert_eq!(result.unwrap(), call);
        }

        #[test]
        fn message_json_roundtrip(msg in arb_message()) {
            let result = json_roundtrip(&msg);
            prop_assert!(result.is_ok(), "JSON roundtrip failed: {:?}", result.err());
            prop_assert_eq!(result.unwrap(), msg);
        }

        #[test]
        fn image_source_json_roundtrip(source in arb_image_source()) {
            let result = json_roundtrip(&source);
            prop_assert!(result.is_ok(), "JSON roundtrip failed: {:?}", result.err());
            prop_assert_eq!(result.unwrap(), source);
        }

        #[test]
        fn image_detail_json_roundtrip(detail in arb_image_detail()) {
            let result = json_roundtrip(&detail);
            prop_assert!(result.is_ok(), "JSON roundtrip failed: {:?}", result.err());
            prop_assert_eq!(result.unwrap(), detail);
        }
    }

    // =========================================================================
    // Property Tests: Graph Registry Types (JSON Stability - no PartialEq)
    // =========================================================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(500))]

        /// Test that FieldDiff JSON serialization is stable (roundtrip produces same JSON)
        #[test]
        fn field_diff_json_stable(diff in arb_field_diff()) {
            let result = json_stable(&diff);
            prop_assert!(result.is_ok(), "JSON not stable: {:?}", result.err());
        }

        /// Test that StateDiff JSON serialization is stable
        #[test]
        fn state_diff_json_stable(diff in arb_state_diff()) {
            let result = json_stable(&diff);
            prop_assert!(result.is_ok(), "JSON not stable: {:?}", result.err());
        }

        // Note: Bincode tests for FieldDiff/StateDiff removed because serde_json::Value
        // does not serialize well with bincode (JSON values have variable structure that
        // doesn't map cleanly to bincode's format). JSON serialization is the primary
        // use case for these types.
    }

    // =========================================================================
    // Property Tests: Checkpoint Integrity Errors (Display)
    // =========================================================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        // CheckpointIntegrityError doesn't derive Serialize/Deserialize
        // but we test that Display doesn't panic and produces useful output
        #[test]
        fn checkpoint_integrity_error_display(err in arb_checkpoint_integrity_error()) {
            let display = format!("{}", err);
            prop_assert!(!display.is_empty());
            // Verify error messages contain relevant information
            match &err {
                CheckpointIntegrityError::FileTooSmall { size, minimum } => {
                    prop_assert!(display.contains(&size.to_string()));
                    prop_assert!(display.contains(&minimum.to_string()));
                }
                CheckpointIntegrityError::InvalidMagic { .. } => {
                    prop_assert!(display.contains("magic") || display.contains("Magic"));
                }
                CheckpointIntegrityError::UnsupportedVersion { found, supported } => {
                    prop_assert!(display.contains(&found.to_string()));
                    prop_assert!(display.contains(&supported.to_string()));
                }
                CheckpointIntegrityError::ChecksumMismatch { .. } => {
                    prop_assert!(display.contains("checksum") || display.contains("Checksum"));
                }
                CheckpointIntegrityError::LengthMismatch { declared, actual } => {
                    prop_assert!(display.contains(&declared.to_string()));
                    prop_assert!(display.contains(&actual.to_string()));
                }
                // Catch-all for #[non_exhaustive] enum - if new variants are added,
                // they should be added to arb_checkpoint_integrity_error() and above
                #[allow(unreachable_patterns)]
                _ => {}
            }
        }
    }

    // =========================================================================
    // Property Tests: Edge Cases
    // =========================================================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        /// Test that empty strings serialize/deserialize correctly
        #[test]
        fn empty_string_in_content_block(_dummy in 0..1i32) {
            let block = ContentBlock::Text { text: String::new() };
            let result = json_roundtrip(&block);
            prop_assert!(result.is_ok());
            prop_assert_eq!(result.unwrap(), block);
        }

        /// Test that empty vectors in messages serialize correctly
        #[test]
        fn empty_tool_calls_in_message(_dummy in 0..1i32) {
            let msg = Message::AI {
                content: MessageContent::Text("test".to_string()),
                tool_calls: vec![],
                invalid_tool_calls: vec![],
                usage_metadata: None,
                fields: BaseMessageFields::default(),
            };
            let result = json_roundtrip(&msg);
            prop_assert!(result.is_ok());
            prop_assert_eq!(result.unwrap(), msg);
        }

        /// Test that unicode content roundtrips correctly
        #[test]
        fn unicode_content_roundtrip(content in "[\u{0020}-\u{007E}\u{00A0}-\u{00FF}\u{4E00}-\u{9FFF}]{0,50}") {
            let block = ContentBlock::Text { text: content.clone() };
            let result = json_roundtrip(&block);
            prop_assert!(result.is_ok(), "Unicode roundtrip failed");
            if let ContentBlock::Text { text } = result.unwrap() {
                prop_assert_eq!(text, content);
            } else {
                prop_assert!(false, "Wrong variant returned");
            }
        }

        /// Test that nested JSON values in tool calls roundtrip correctly
        #[test]
        fn nested_json_in_tool_call(
            id in valid_id(),
            name in valid_id(),
            nested_key in valid_id(),
            nested_val in valid_string()
        ) {
            let args = serde_json::json!({
                "outer": {
                    nested_key: nested_val
                }
            });
            let call = ToolCall {
                id,
                name,
                args,
                tool_type: "tool_call".to_string(),
                index: None,
            };
            let result = json_roundtrip(&call);
            prop_assert!(result.is_ok());
            prop_assert_eq!(result.unwrap(), call);
        }
    }

    // =========================================================================
    // Deterministic Tests for Known Edge Cases
    // =========================================================================

    #[test]
    fn test_content_block_all_variants() {
        let variants = vec![
            ContentBlock::Text {
                text: "hello".to_string(),
            },
            ContentBlock::Image {
                source: ImageSource::Url {
                    url: "https://example.com/image.png".to_string(),
                },
                detail: Some(ImageDetail::High),
            },
            ContentBlock::Image {
                source: ImageSource::Base64 {
                    media_type: "image/png".to_string(),
                    data: "iVBORw0KGgo=".to_string(),
                },
                detail: None,
            },
            ContentBlock::ToolUse {
                id: "tool_1".to_string(),
                name: "calculator".to_string(),
                input: serde_json::json!({"expression": "2+2"}),
            },
            ContentBlock::ToolResult {
                tool_use_id: "tool_1".to_string(),
                content: "4".to_string(),
                is_error: false,
            },
            ContentBlock::Reasoning {
                reasoning: "Let me think...".to_string(),
            },
            ContentBlock::Thinking {
                thinking: "Deep thought".to_string(),
                signature: Some("sig123".to_string()),
            },
            ContentBlock::RedactedThinking {
                data: "redacted".to_string(),
            },
        ];

        for variant in variants {
            let json = serde_json::to_string(&variant).expect("serialize should work");
            let roundtrip: ContentBlock =
                serde_json::from_str(&json).expect("deserialize should work");
            assert_eq!(roundtrip, variant, "Variant didn't roundtrip correctly");
        }
    }

    #[test]
    fn test_message_all_variants() {
        let variants = vec![
            Message::Human {
                content: MessageContent::Text("Hello".to_string()),
                fields: BaseMessageFields::default(),
            },
            Message::AI {
                content: MessageContent::Text("Hi there!".to_string()),
                tool_calls: vec![],
                invalid_tool_calls: vec![],
                usage_metadata: Some(UsageMetadata {
                    input_tokens: 10,
                    output_tokens: 5,
                    total_tokens: 15,
                    ..Default::default()
                }),
                fields: BaseMessageFields::default(),
            },
            Message::System {
                content: MessageContent::Text("You are helpful.".to_string()),
                fields: BaseMessageFields::default(),
            },
            Message::Tool {
                content: MessageContent::Text("Result: 42".to_string()),
                tool_call_id: "call_123".to_string(),
                artifact: None,
                status: None,
                fields: BaseMessageFields::default(),
            },
        ];

        for variant in variants {
            let json = serde_json::to_string(&variant).expect("serialize should work");
            let roundtrip: Message = serde_json::from_str(&json).expect("deserialize should work");
            assert_eq!(
                roundtrip, variant,
                "Message variant didn't roundtrip correctly"
            );
        }
    }

    #[test]
    fn test_state_diff_empty() {
        let diff = StateDiff::empty();
        let json = serde_json::to_string(&diff).expect("serialize should work");
        let roundtrip: StateDiff = serde_json::from_str(&json).expect("deserialize should work");
        assert!(
            roundtrip.added.is_empty()
                && roundtrip.removed.is_empty()
                && roundtrip.modified.is_empty()
        );
    }

    #[test]
    fn test_state_diff_with_changes() {
        let diff = StateDiff {
            added: vec!["new_field".to_string()],
            removed: vec!["old_field".to_string()],
            modified: vec![FieldDiff {
                path: "count".to_string(),
                before: serde_json::json!(1),
                after: serde_json::json!(2),
            }],
        };
        let json = serde_json::to_string(&diff).expect("serialize should work");
        let roundtrip: StateDiff = serde_json::from_str(&json).expect("deserialize should work");

        // Compare fields manually since StateDiff doesn't impl PartialEq
        assert_eq!(roundtrip.added, diff.added);
        assert_eq!(roundtrip.removed, diff.removed);
        assert_eq!(roundtrip.modified.len(), diff.modified.len());
        assert_eq!(roundtrip.modified[0].path, diff.modified[0].path);
        assert_eq!(roundtrip.modified[0].before, diff.modified[0].before);
        assert_eq!(roundtrip.modified[0].after, diff.modified[0].after);
    }

    #[test]
    fn test_field_diff_with_complex_json() {
        let diff = FieldDiff {
            path: "nested.array[0].value".to_string(),
            before: serde_json::json!({
                "key": "old_value",
                "array": [1, 2, 3]
            }),
            after: serde_json::json!({
                "key": "new_value",
                "array": [1, 2, 3, 4],
                "added": true
            }),
        };
        let json = serde_json::to_string(&diff).expect("serialize should work");
        let roundtrip: FieldDiff = serde_json::from_str(&json).expect("deserialize should work");

        assert_eq!(roundtrip.path, diff.path);
        assert_eq!(roundtrip.before, diff.before);
        assert_eq!(roundtrip.after, diff.after);
    }

    // Note: test_bincode_state_diff removed because serde_json::Value doesn't
    // serialize reliably with bincode (JSON values have variable structure).
    // JSON serialization is the primary use case for StateDiff/FieldDiff.

    #[test]
    fn test_message_with_blocks_content() {
        let msg = Message::Human {
            content: MessageContent::Blocks(vec![
                ContentBlock::Text {
                    text: "Hello, ".to_string(),
                },
                ContentBlock::Text {
                    text: "world!".to_string(),
                },
            ]),
            fields: BaseMessageFields::default(),
        };

        let json = serde_json::to_string(&msg).expect("serialize should work");
        let roundtrip: Message = serde_json::from_str(&json).expect("deserialize should work");
        assert_eq!(roundtrip, msg);
    }

    #[test]
    fn test_tool_call_with_null_args() {
        let call = ToolCall {
            id: "test".to_string(),
            name: "no_args".to_string(),
            args: serde_json::Value::Null,
            tool_type: "tool_call".to_string(),
            index: None,
        };

        let json = serde_json::to_string(&call).expect("serialize should work");
        let roundtrip: ToolCall = serde_json::from_str(&json).expect("deserialize should work");
        assert_eq!(roundtrip, call);
    }

    #[test]
    fn test_usage_metadata_zero_values() {
        let usage = UsageMetadata {
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            ..Default::default()
        };

        let json = serde_json::to_string(&usage).expect("serialize should work");
        let roundtrip: UsageMetadata =
            serde_json::from_str(&json).expect("deserialize should work");

        // UsageMetadata has PartialEq
        assert_eq!(roundtrip, usage);
    }
}
