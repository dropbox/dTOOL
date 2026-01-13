//! Integration tests for GraphState derive macro
//!
//! These tests must be in an integration test file (not unit tests)
//! because the macro needs to reference `::dashflow::` which
//! isn't available in the crate's own unit tests.

use dashflow::core::messages::Message;
use dashflow::reducer::MessageExt;
use dashflow::GraphStateDerive;
use dashflow::MergeableState;
use serde::{Deserialize, Serialize};

#[test]
fn test_graph_state_derive_macro() {
    #[derive(Clone, Serialize, Deserialize, GraphStateDerive)]
    struct TestState {
        #[add_messages]
        messages: Vec<Message>,
        counter: i32,
    }

    impl MergeableState for TestState {
        fn merge(&mut self, other: &Self) {
            self.messages.extend(other.messages.clone());
            self.counter = self.counter.max(other.counter);
        }
    }

    let state1 = TestState {
        messages: vec![Message::human("Hello").with_id("msg1")],
        counter: 5,
    };

    let state2 = TestState {
        messages: vec![
            Message::human("Hello updated").with_id("msg1"),
            Message::ai("Hi").with_id("msg2"),
        ],
        counter: 10,
    };

    let merged = state1.merge_partial(&state2);

    // Messages should be merged with add_messages reducer
    assert_eq!(merged.messages.len(), 2);
    assert_eq!(merged.messages[0].as_text(), "Hello updated");
    assert_eq!(merged.messages[1].as_text(), "Hi");

    // Counter should use right-side value (no reducer attribute)
    assert_eq!(merged.counter, 10);
}

#[test]
fn test_graph_state_custom_reducer() {
    #[allow(clippy::needless_pass_by_value)]
    fn concat_strings(left: String, right: String) -> String {
        if left.is_empty() {
            right
        } else {
            format!("{}\n{}", left, right)
        }
    }

    #[derive(Clone, Serialize, Deserialize, GraphStateDerive)]
    struct TestState {
        #[reducer(concat_strings)]
        log: String,
        value: i32,
    }

    let state1 = TestState {
        log: "Line 1".to_string(),
        value: 5,
    };

    let state2 = TestState {
        log: "Line 2".to_string(),
        value: 10,
    };

    let merged = state1.merge_partial(&state2);

    // Log should use custom concat_strings reducer
    assert_eq!(merged.log, "Line 1\nLine 2");

    // Value should use right-side value
    assert_eq!(merged.value, 10);
}

#[test]
fn test_graph_state_no_reducer() {
    #[derive(Clone, Serialize, Deserialize, GraphStateDerive)]
    struct TestState {
        value: i32,
        text: String,
    }

    let state1 = TestState {
        value: 5,
        text: "first".to_string(),
    };

    let state2 = TestState {
        value: 10,
        text: "second".to_string(),
    };

    let merged = state1.merge_partial(&state2);

    // Without reducers, right side wins
    assert_eq!(merged.value, 10);
    assert_eq!(merged.text, "second");
}
