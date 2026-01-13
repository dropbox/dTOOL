use dashflow::MergeableState as MergeableStateTrait;
use dashflow_derive::{GraphState, MergeableState};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

/// Test that GraphState derive compiles for a simple struct
///
/// Test infrastructure struct: Validates GraphState derive macro compilation
#[derive(Clone, Serialize, Deserialize, GraphState)]
#[allow(dead_code)] // Test: Struct validates derive macro compiles for basic types
struct SimpleState {
    messages: Vec<String>,
    count: usize,
}

/// Test that MergeableState derive generates correct merge logic for Vec fields
#[test]
fn test_mergeable_state_vec_extend() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        items: Vec<String>,
    }

    let mut state1 = TestState {
        items: vec!["a".to_string(), "b".to_string()],
    };

    let state2 = TestState {
        items: vec!["c".to_string(), "d".to_string()],
    };

    state1.merge(&state2);

    assert_eq!(state1.items.len(), 4);
    assert_eq!(state1.items, vec!["a", "b", "c", "d"]);
}

/// Test that MergeableState derive generates correct merge logic for numeric fields
#[test]
fn test_mergeable_state_numeric_max() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        max_value: i32,
        count: usize,
    }

    let mut state1 = TestState {
        max_value: 5,
        count: 10,
    };

    let state2 = TestState {
        max_value: 8,
        count: 7,
    };

    state1.merge(&state2);

    assert_eq!(state1.max_value, 8); // Takes max
    assert_eq!(state1.count, 10); // Takes max
}

/// Test that MergeableState derive generates correct merge logic for String fields
#[test]
fn test_mergeable_state_string_concat() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        description: String,
    }

    let mut state1 = TestState {
        description: "First line".to_string(),
    };

    let state2 = TestState {
        description: "Second line".to_string(),
    };

    state1.merge(&state2);

    assert_eq!(state1.description, "First line\nSecond line");
}

/// Test that MergeableState derive handles Option fields correctly
#[test]
fn test_mergeable_state_option() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        optional_value: Option<String>,
    }

    let mut state1 = TestState {
        optional_value: None,
    };

    let state2 = TestState {
        optional_value: Some("value".to_string()),
    };

    state1.merge(&state2);
    assert_eq!(state1.optional_value, Some("value".to_string()));

    // If state1 already has a value, it should keep it
    let mut state3 = TestState {
        optional_value: Some("existing".to_string()),
    };

    state3.merge(&state2);
    assert_eq!(state3.optional_value, Some("existing".to_string()));
}

/// Test that MergeableState works with complex nested structures
#[test]
fn test_mergeable_state_complex() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct ComplexState {
        findings: Vec<String>,
        insights: Vec<String>,
        total_score: i32,
        summary: String,
    }

    let mut state1 = ComplexState {
        findings: vec!["finding1".to_string()],
        insights: vec!["insight1".to_string()],
        total_score: 42,
        summary: "First summary".to_string(),
    };

    let state2 = ComplexState {
        findings: vec!["finding2".to_string(), "finding3".to_string()],
        insights: vec!["insight2".to_string()],
        total_score: 58,
        summary: "Second summary".to_string(),
    };

    state1.merge(&state2);

    // Vec fields should extend
    assert_eq!(state1.findings.len(), 3);
    assert_eq!(state1.insights.len(), 2);

    // Numeric fields should take max
    assert_eq!(state1.total_score, 58);

    // String fields should concatenate
    assert_eq!(state1.summary, "First summary\nSecond summary");
}

/// Test that MergeableState handles HashSet fields correctly
#[test]
fn test_mergeable_state_hashset() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        tags: HashSet<String>,
    }

    let mut state1 = TestState {
        tags: ["tag1".to_string(), "tag2".to_string()]
            .iter()
            .cloned()
            .collect(),
    };

    let state2 = TestState {
        tags: ["tag2".to_string(), "tag3".to_string()]
            .iter()
            .cloned()
            .collect(),
    };

    state1.merge(&state2);

    // HashSet should merge and deduplicate
    assert_eq!(state1.tags.len(), 3);
    assert!(state1.tags.contains("tag1"));
    assert!(state1.tags.contains("tag2"));
    assert!(state1.tags.contains("tag3"));
}

/// Test that MergeableState handles HashMap fields correctly
#[test]
fn test_mergeable_state_hashmap() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        metadata: HashMap<String, String>,
    }

    let mut state1 = TestState {
        metadata: [
            ("key1".to_string(), "value1".to_string()),
            ("key2".to_string(), "value2".to_string()),
        ]
        .iter()
        .cloned()
        .collect(),
    };

    let state2 = TestState {
        metadata: [
            ("key2".to_string(), "value2_updated".to_string()),
            ("key3".to_string(), "value3".to_string()),
        ]
        .iter()
        .cloned()
        .collect(),
    };

    state1.merge(&state2);

    // HashMap should merge and overwrite on key collision
    assert_eq!(state1.metadata.len(), 3);
    assert_eq!(state1.metadata.get("key1"), Some(&"value1".to_string()));
    assert_eq!(
        state1.metadata.get("key2"),
        Some(&"value2_updated".to_string())
    );
    assert_eq!(state1.metadata.get("key3"), Some(&"value3".to_string()));
}

/// Test that MergeableState handles bool fields correctly (logical OR)
#[test]
fn test_mergeable_state_bool() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        enabled: bool,
        active: bool,
    }

    let mut state1 = TestState {
        enabled: false,
        active: true,
    };

    let state2 = TestState {
        enabled: true,
        active: false,
    };

    state1.merge(&state2);

    // bool fields should use logical OR
    assert!(state1.enabled); // false || true = true
    assert!(state1.active); // true || false = true
}

/// Test comprehensive state with all supported types
#[test]
fn test_mergeable_state_all_types() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct ComprehensiveState {
        items: Vec<String>,
        tags: HashSet<String>,
        metadata: HashMap<String, i32>,
        optional_value: Option<String>,
        description: String,
        max_score: i32,
        is_complete: bool,
    }

    let mut state1 = ComprehensiveState {
        items: vec!["item1".to_string()],
        tags: ["tag1".to_string()].iter().cloned().collect(),
        metadata: [("key1".to_string(), 10)].iter().cloned().collect(),
        optional_value: None,
        description: "First".to_string(),
        max_score: 50,
        is_complete: false,
    };

    let state2 = ComprehensiveState {
        items: vec!["item2".to_string()],
        tags: ["tag2".to_string()].iter().cloned().collect(),
        metadata: [("key2".to_string(), 20)].iter().cloned().collect(),
        optional_value: Some("value".to_string()),
        description: "Second".to_string(),
        max_score: 75,
        is_complete: true,
    };

    state1.merge(&state2);

    // Verify all merge behaviors
    assert_eq!(state1.items, vec!["item1", "item2"]);
    assert_eq!(state1.tags.len(), 2);
    assert!(state1.tags.contains("tag1"));
    assert!(state1.tags.contains("tag2"));
    assert_eq!(state1.metadata.len(), 2);
    assert_eq!(state1.metadata.get("key1"), Some(&10));
    assert_eq!(state1.metadata.get("key2"), Some(&20));
    assert_eq!(state1.optional_value, Some("value".to_string()));
    assert_eq!(state1.description, "First\nSecond");
    assert_eq!(state1.max_score, 75);
    assert!(state1.is_complete);
}

/// Test that MergeableState handles VecDeque fields correctly
#[test]
fn test_mergeable_state_vecdeque() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        queue: VecDeque<String>,
    }

    let mut state1 = TestState {
        queue: ["item1".to_string(), "item2".to_string()]
            .iter()
            .cloned()
            .collect(),
    };

    let state2 = TestState {
        queue: ["item3".to_string(), "item4".to_string()]
            .iter()
            .cloned()
            .collect(),
    };

    state1.merge(&state2);

    // VecDeque should extend with other's elements
    assert_eq!(state1.queue.len(), 4);
    let items: Vec<String> = state1.queue.iter().cloned().collect();
    assert_eq!(items, vec!["item1", "item2", "item3", "item4"]);
}

/// Test that MergeableState handles BTreeSet fields correctly
#[test]
fn test_mergeable_state_btreeset() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        ordered_tags: BTreeSet<String>,
    }

    let mut state1 = TestState {
        ordered_tags: ["tag1".to_string(), "tag2".to_string()]
            .iter()
            .cloned()
            .collect(),
    };

    let state2 = TestState {
        ordered_tags: ["tag2".to_string(), "tag3".to_string()]
            .iter()
            .cloned()
            .collect(),
    };

    state1.merge(&state2);

    // BTreeSet should merge, deduplicate, and maintain sorted order
    assert_eq!(state1.ordered_tags.len(), 3);
    assert!(state1.ordered_tags.contains("tag1"));
    assert!(state1.ordered_tags.contains("tag2"));
    assert!(state1.ordered_tags.contains("tag3"));

    // Verify sorted order
    let sorted: Vec<String> = state1.ordered_tags.iter().cloned().collect();
    assert_eq!(sorted, vec!["tag1", "tag2", "tag3"]);
}

/// Test that MergeableState handles BTreeMap fields correctly
#[test]
fn test_mergeable_state_btreemap() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        ordered_metadata: BTreeMap<String, i32>,
    }

    let mut state1 = TestState {
        ordered_metadata: [("key1".to_string(), 10), ("key2".to_string(), 20)]
            .iter()
            .cloned()
            .collect(),
    };

    let state2 = TestState {
        ordered_metadata: [("key2".to_string(), 25), ("key3".to_string(), 30)]
            .iter()
            .cloned()
            .collect(),
    };

    state1.merge(&state2);

    // BTreeMap should merge and overwrite on key collision
    assert_eq!(state1.ordered_metadata.len(), 3);
    assert_eq!(state1.ordered_metadata.get("key1"), Some(&10));
    assert_eq!(state1.ordered_metadata.get("key2"), Some(&25)); // Overwritten
    assert_eq!(state1.ordered_metadata.get("key3"), Some(&30));

    // Verify sorted order by keys
    let keys: Vec<String> = state1.ordered_metadata.keys().cloned().collect();
    assert_eq!(keys, vec!["key1", "key2", "key3"]);
}

/// Test comprehensive state with all supported collection types including new ones
#[test]
fn test_mergeable_state_all_collections() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct AllCollectionsState {
        vec_items: Vec<String>,
        deque_items: VecDeque<String>,
        hash_tags: HashSet<String>,
        btree_tags: BTreeSet<String>,
        hash_metadata: HashMap<String, i32>,
        btree_metadata: BTreeMap<String, i32>,
    }

    let mut state1 = AllCollectionsState {
        vec_items: vec!["v1".to_string()],
        deque_items: ["d1".to_string()].iter().cloned().collect(),
        hash_tags: ["h1".to_string()].iter().cloned().collect(),
        btree_tags: ["b1".to_string()].iter().cloned().collect(),
        hash_metadata: [("hk1".to_string(), 1)].iter().cloned().collect(),
        btree_metadata: [("bk1".to_string(), 1)].iter().cloned().collect(),
    };

    let state2 = AllCollectionsState {
        vec_items: vec!["v2".to_string()],
        deque_items: ["d2".to_string()].iter().cloned().collect(),
        hash_tags: ["h2".to_string()].iter().cloned().collect(),
        btree_tags: ["b2".to_string()].iter().cloned().collect(),
        hash_metadata: [("hk2".to_string(), 2)].iter().cloned().collect(),
        btree_metadata: [("bk2".to_string(), 2)].iter().cloned().collect(),
    };

    state1.merge(&state2);

    // Verify all collections merged correctly
    assert_eq!(state1.vec_items, vec!["v1", "v2"]);
    assert_eq!(state1.deque_items.len(), 2);
    assert_eq!(state1.hash_tags.len(), 2);
    assert_eq!(state1.btree_tags.len(), 2);
    assert_eq!(state1.hash_metadata.len(), 2);
    assert_eq!(state1.btree_metadata.len(), 2);
}

// ============================================================================
// Tests for #[merge(...)] field attributes
// ============================================================================

/// Test #[merge(skip)] attribute - field should not be merged
#[test]
fn test_merge_attribute_skip() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        #[merge(skip)]
        immutable_id: String,
        mutable_value: String,
    }

    let mut state1 = TestState {
        immutable_id: "original_id".to_string(),
        mutable_value: "first".to_string(),
    };

    let state2 = TestState {
        immutable_id: "new_id".to_string(),
        mutable_value: "second".to_string(),
    };

    state1.merge(&state2);

    // immutable_id should NOT change (skip)
    assert_eq!(state1.immutable_id, "original_id");
    // mutable_value should be concatenated (default String behavior)
    assert_eq!(state1.mutable_value, "first\nsecond");
}

/// Test #[merge(skip)] with numeric field
#[test]
fn test_merge_attribute_skip_numeric() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        #[merge(skip)]
        version: i32,
        counter: i32,
    }

    let mut state1 = TestState {
        version: 1,
        counter: 5,
    };

    let state2 = TestState {
        version: 2,
        counter: 10,
    };

    state1.merge(&state2);

    // version should NOT change (skip)
    assert_eq!(state1.version, 1);
    // counter should take max (default numeric behavior)
    assert_eq!(state1.counter, 10);
}

/// Test #[merge(replace)] attribute - replace if other is non-empty
#[test]
fn test_merge_attribute_replace_string() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        #[merge(replace)]
        status: String,
        description: String,
    }

    let mut state1 = TestState {
        status: "pending".to_string(),
        description: "first".to_string(),
    };

    let state2 = TestState {
        status: "completed".to_string(),
        description: "second".to_string(),
    };

    state1.merge(&state2);

    // status should be replaced (not concatenated)
    assert_eq!(state1.status, "completed");
    // description should be concatenated (default behavior)
    assert_eq!(state1.description, "first\nsecond");
}

/// Test #[merge(replace)] with empty other value - should keep original
#[test]
fn test_merge_attribute_replace_empty() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        #[merge(replace)]
        status: String,
    }

    let mut state1 = TestState {
        status: "pending".to_string(),
    };

    let state2 = TestState {
        status: String::new(),
    };

    state1.merge(&state2);

    // status should NOT be replaced since other is empty
    assert_eq!(state1.status, "pending");
}

/// Test #[merge(replace)] with Vec field
#[test]
fn test_merge_attribute_replace_vec() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        #[merge(replace)]
        items: Vec<String>,
        other_items: Vec<String>,
    }

    let mut state1 = TestState {
        items: vec!["a".to_string(), "b".to_string()],
        other_items: vec!["x".to_string()],
    };

    let state2 = TestState {
        items: vec!["c".to_string()],
        other_items: vec!["y".to_string()],
    };

    state1.merge(&state2);

    // items should be replaced completely (not extended)
    assert_eq!(state1.items, vec!["c"]);
    // other_items should be extended (default Vec behavior)
    assert_eq!(state1.other_items, vec!["x", "y"]);
}

/// Test #[merge(replace)] with Option field
#[test]
fn test_merge_attribute_replace_option() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        #[merge(replace)]
        status: Option<String>,
        default_option: Option<String>,
    }

    let mut state1 = TestState {
        status: Some("initial".to_string()),
        default_option: None,
    };

    let state2 = TestState {
        status: Some("updated".to_string()),
        default_option: Some("value".to_string()),
    };

    state1.merge(&state2);

    // status should be replaced even though self had a value
    assert_eq!(state1.status, Some("updated".to_string()));
    // default_option should take other's value (default Option behavior)
    assert_eq!(state1.default_option, Some("value".to_string()));
}

/// Test #[merge(take_if_empty)] attribute - only take other if self is empty
#[test]
fn test_merge_attribute_take_if_empty_string() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        #[merge(take_if_empty)]
        default_value: String,
        regular_value: String,
    }

    // Test 1: self is empty, should take other
    let mut state1 = TestState {
        default_value: String::new(),
        regular_value: String::new(),
    };

    let state2 = TestState {
        default_value: "provided".to_string(),
        regular_value: "other".to_string(),
    };

    state1.merge(&state2);

    assert_eq!(state1.default_value, "provided");
    assert_eq!(state1.regular_value, "other");
}

/// Test #[merge(take_if_empty)] when self is not empty - should keep original
#[test]
fn test_merge_attribute_take_if_empty_not_empty() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        #[merge(take_if_empty)]
        default_value: String,
    }

    let mut state1 = TestState {
        default_value: "existing".to_string(),
    };

    let state2 = TestState {
        default_value: "new".to_string(),
    };

    state1.merge(&state2);

    // Should keep existing value
    assert_eq!(state1.default_value, "existing");
}

/// Test #[merge(take_if_empty)] with Vec field
#[test]
fn test_merge_attribute_take_if_empty_vec() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        #[merge(take_if_empty)]
        initial_items: Vec<String>,
    }

    // Test 1: self is empty
    let mut state1 = TestState {
        initial_items: vec![],
    };

    let state2 = TestState {
        initial_items: vec!["a".to_string(), "b".to_string()],
    };

    state1.merge(&state2);

    assert_eq!(state1.initial_items, vec!["a", "b"]);

    // Test 2: self is not empty - should keep original
    let mut state3 = TestState {
        initial_items: vec!["x".to_string()],
    };

    state3.merge(&state2);

    assert_eq!(state3.initial_items, vec!["x"]);
}

/// Test #[merge(take_if_empty)] with Option field
#[test]
fn test_merge_attribute_take_if_empty_option() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        #[merge(take_if_empty)]
        optional_value: Option<String>,
    }

    // Test 1: self is None
    let mut state1 = TestState {
        optional_value: None,
    };

    let state2 = TestState {
        optional_value: Some("provided".to_string()),
    };

    state1.merge(&state2);

    assert_eq!(state1.optional_value, Some("provided".to_string()));

    // Test 2: self is Some - should keep original
    let mut state3 = TestState {
        optional_value: Some("existing".to_string()),
    };

    state3.merge(&state2);

    assert_eq!(state3.optional_value, Some("existing".to_string()));
}

/// Test #[merge(recursive)] attribute - call merge() on nested MergeableState
#[test]
fn test_merge_attribute_recursive() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct InnerState {
        items: Vec<String>,
        count: i32,
    }

    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct OuterState {
        #[merge(recursive)]
        inner: InnerState,
        outer_value: String,
    }

    let mut state1 = OuterState {
        inner: InnerState {
            items: vec!["a".to_string()],
            count: 5,
        },
        outer_value: "first".to_string(),
    };

    let state2 = OuterState {
        inner: InnerState {
            items: vec!["b".to_string()],
            count: 10,
        },
        outer_value: "second".to_string(),
    };

    state1.merge(&state2);

    // inner should be recursively merged
    assert_eq!(state1.inner.items, vec!["a", "b"]);
    assert_eq!(state1.inner.count, 10);
    // outer_value should use default String behavior
    assert_eq!(state1.outer_value, "first\nsecond");
}

/// Test combining multiple merge attributes in one struct
#[test]
fn test_merge_attributes_combined() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        #[merge(skip)]
        id: String,
        #[merge(replace)]
        status: String,
        #[merge(take_if_empty)]
        initial_value: String,
        regular_items: Vec<String>,
    }

    let mut state1 = TestState {
        id: "original".to_string(),
        status: "pending".to_string(),
        initial_value: String::new(),
        regular_items: vec!["a".to_string()],
    };

    let state2 = TestState {
        id: "new_id".to_string(),
        status: "completed".to_string(),
        initial_value: "default".to_string(),
        regular_items: vec!["b".to_string()],
    };

    state1.merge(&state2);

    assert_eq!(state1.id, "original"); // skip
    assert_eq!(state1.status, "completed"); // replace
    assert_eq!(state1.initial_value, "default"); // take_if_empty
    assert_eq!(state1.regular_items, vec!["a", "b"]); // extend
}

// ============================================================================
// Edge case tests
// ============================================================================

/// Test merging empty strings
#[test]
fn test_merge_empty_strings() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        description: String,
    }

    // Test 1: Both empty
    let mut state1 = TestState {
        description: String::new(),
    };
    let state2 = TestState {
        description: String::new(),
    };
    state1.merge(&state2);
    assert_eq!(state1.description, "");

    // Test 2: Self non-empty, other empty
    let mut state3 = TestState {
        description: "existing".to_string(),
    };
    let state4 = TestState {
        description: String::new(),
    };
    state3.merge(&state4);
    assert_eq!(state3.description, "existing");

    // Test 3: Self empty, other non-empty
    let mut state5 = TestState {
        description: String::new(),
    };
    let state6 = TestState {
        description: "new".to_string(),
    };
    state5.merge(&state6);
    assert_eq!(state5.description, "new");
}

/// Test merging empty vectors
#[test]
fn test_merge_empty_vecs() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        items: Vec<String>,
    }

    // Test 1: Both empty
    let mut state1 = TestState { items: vec![] };
    let state2 = TestState { items: vec![] };
    state1.merge(&state2);
    assert!(state1.items.is_empty());

    // Test 2: Self non-empty, other empty
    let mut state3 = TestState {
        items: vec!["a".to_string()],
    };
    let state4 = TestState { items: vec![] };
    state3.merge(&state4);
    assert_eq!(state3.items, vec!["a"]);

    // Test 3: Self empty, other non-empty
    let mut state5 = TestState { items: vec![] };
    let state6 = TestState {
        items: vec!["b".to_string()],
    };
    state5.merge(&state6);
    assert_eq!(state5.items, vec!["b"]);
}

/// Test merging when both Options have values
#[test]
fn test_merge_both_options_some() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        optional_value: Option<String>,
    }

    let mut state1 = TestState {
        optional_value: Some("first".to_string()),
    };

    let state2 = TestState {
        optional_value: Some("second".to_string()),
    };

    state1.merge(&state2);

    // Default behavior: keep self's value if both are Some
    assert_eq!(state1.optional_value, Some("first".to_string()));
}

/// Test merging when both Options are None
#[test]
fn test_merge_both_options_none() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        optional_value: Option<String>,
    }

    let mut state1 = TestState {
        optional_value: None,
    };

    let state2 = TestState {
        optional_value: None,
    };

    state1.merge(&state2);

    assert_eq!(state1.optional_value, None);
}

/// Test merging numeric edge cases (zero, max values)
#[test]
fn test_merge_numeric_edge_cases() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        count: i32,
        size: usize,
    }

    // Test with zero
    let mut state1 = TestState { count: 0, size: 0 };
    let state2 = TestState { count: 5, size: 10 };
    state1.merge(&state2);
    assert_eq!(state1.count, 5);
    assert_eq!(state1.size, 10);

    // Test when other is smaller
    let mut state3 = TestState {
        count: 100,
        size: 200,
    };
    let state4 = TestState { count: 50, size: 100 };
    state3.merge(&state4);
    assert_eq!(state3.count, 100); // max
    assert_eq!(state3.size, 200); // max

    // Test negative numbers
    let mut state5 = TestState {
        count: -10,
        size: 0,
    };
    let state6 = TestState { count: -5, size: 0 };
    state5.merge(&state6);
    assert_eq!(state5.count, -5); // max(-10, -5) = -5
}

/// Test merging bool edge cases
#[test]
fn test_merge_bool_edge_cases() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        flag1: bool,
        flag2: bool,
        flag3: bool,
        flag4: bool,
    }

    let mut state1 = TestState {
        flag1: false,
        flag2: false,
        flag3: true,
        flag4: true,
    };

    let state2 = TestState {
        flag1: false,
        flag2: true,
        flag3: false,
        flag4: true,
    };

    state1.merge(&state2);

    assert!(!state1.flag1); // false || false = false
    assert!(state1.flag2); // false || true = true
    assert!(state1.flag3); // true || false = true
    assert!(state1.flag4); // true || true = true
}

/// Test merging with multiline strings
#[test]
fn test_merge_multiline_strings() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        description: String,
    }

    let mut state1 = TestState {
        description: "Line 1\nLine 2".to_string(),
    };

    let state2 = TestState {
        description: "Line 3\nLine 4".to_string(),
    };

    state1.merge(&state2);

    assert_eq!(state1.description, "Line 1\nLine 2\nLine 3\nLine 4");
}

/// Test merging HashMap with overlapping and non-overlapping keys
#[test]
fn test_merge_hashmap_key_collision() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        metadata: HashMap<String, i32>,
    }

    let mut state1 = TestState {
        metadata: [
            ("a".to_string(), 1),
            ("b".to_string(), 2),
            ("c".to_string(), 3),
        ]
        .iter()
        .cloned()
        .collect(),
    };

    let state2 = TestState {
        metadata: [
            ("b".to_string(), 20), // collision - should overwrite
            ("c".to_string(), 30), // collision - should overwrite
            ("d".to_string(), 4),  // new key
        ]
        .iter()
        .cloned()
        .collect(),
    };

    state1.merge(&state2);

    assert_eq!(state1.metadata.len(), 4);
    assert_eq!(state1.metadata.get("a"), Some(&1)); // unchanged
    assert_eq!(state1.metadata.get("b"), Some(&20)); // overwritten
    assert_eq!(state1.metadata.get("c"), Some(&30)); // overwritten
    assert_eq!(state1.metadata.get("d"), Some(&4)); // new
}

/// Test merging HashSet with duplicates
#[test]
fn test_merge_hashset_duplicates() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        tags: HashSet<String>,
    }

    let mut state1 = TestState {
        tags: ["a".to_string(), "b".to_string(), "c".to_string()]
            .iter()
            .cloned()
            .collect(),
    };

    let state2 = TestState {
        tags: ["b".to_string(), "c".to_string(), "d".to_string()]
            .iter()
            .cloned()
            .collect(),
    };

    state1.merge(&state2);

    // Should have 4 unique items
    assert_eq!(state1.tags.len(), 4);
    assert!(state1.tags.contains("a"));
    assert!(state1.tags.contains("b"));
    assert!(state1.tags.contains("c"));
    assert!(state1.tags.contains("d"));
}

/// Test all numeric types for merge
#[test]
fn test_merge_all_numeric_types() {
    #[derive(Clone, Serialize, Deserialize, MergeableState)]
    struct TestState {
        val_i8: i8,
        val_u8: u8,
        val_i16: i16,
        val_u16: u16,
        val_i32: i32,
        val_u32: u32,
        val_i64: i64,
        val_u64: u64,
        val_isize: isize,
        val_usize: usize,
        val_f32: f32,
        val_f64: f64,
    }

    let mut state1 = TestState {
        val_i8: -10,
        val_u8: 10,
        val_i16: -100,
        val_u16: 100,
        val_i32: -1000,
        val_u32: 1000,
        val_i64: -10000,
        val_u64: 10000,
        val_isize: -1,
        val_usize: 1,
        val_f32: 1.5,
        val_f64: 2.5,
    };

    let state2 = TestState {
        val_i8: -5,
        val_u8: 20,
        val_i16: -50,
        val_u16: 200,
        val_i32: -500,
        val_u32: 2000,
        val_i64: -5000,
        val_u64: 20000,
        val_isize: -2,
        val_usize: 2,
        val_f32: 0.5,
        val_f64: 3.5,
    };

    state1.merge(&state2);

    // All should take max value
    assert_eq!(state1.val_i8, -5);
    assert_eq!(state1.val_u8, 20);
    assert_eq!(state1.val_i16, -50);
    assert_eq!(state1.val_u16, 200);
    assert_eq!(state1.val_i32, -500);
    assert_eq!(state1.val_u32, 2000);
    assert_eq!(state1.val_i64, -5000);
    assert_eq!(state1.val_u64, 20000);
    assert_eq!(state1.val_isize, -1);
    assert_eq!(state1.val_usize, 2);
    assert!((state1.val_f32 - 1.5).abs() < f32::EPSILON);
    assert!((state1.val_f64 - 3.5).abs() < f64::EPSILON);
}

// Note: DashFlowTool tests are omitted because the derive macro has a pre-existing
// bug where it generates error types that don't exist in the public API.
// The DashFlowTool macro needs to be updated to use dashflow::Error variants
// that are actually exported (e.g., Other(String) instead of ToolExecution).
