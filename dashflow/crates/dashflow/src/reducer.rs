// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! State field reducers for merging updates
//!
//! Reducers define how state fields are merged when a node returns partial state updates.
//! This module provides built-in reducers like `add_messages` for message list merging.

use crate::core::messages::Message;
use uuid::Uuid;

/// Trait for reducing (merging) state field updates
///
/// A reducer takes an old value and a new value, and returns the merged result.
/// This is used during graph execution to combine node outputs with existing state.
pub trait Reducer<T> {
    /// Merge left (current) and right (update) values
    fn reduce(&self, left: T, right: T) -> T;
}

/// Add-messages reducer for `Vec<Message>`
///
/// This reducer implements upstream DashFlow's `add_messages` semantics:
/// - Appends new messages to the list
/// - Updates existing messages by ID (if IDs match)
/// - Automatically assigns UUIDs to messages without IDs
///
/// # Example
/// ```
/// use dashflow::core::messages::Message;
/// use dashflow::reducer::{add_messages, AddMessagesReducer, MessageExt, Reducer};
///
/// let left = vec![Message::human("Hello").with_id("msg1")];
/// let right = vec![Message::ai("Hi there!").with_id("msg2")];
///
/// let reducer = AddMessagesReducer;
/// let merged = reducer.reduce(left, right);
/// assert_eq!(merged.len(), 2);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct AddMessagesReducer;

impl Reducer<Vec<Message>> for AddMessagesReducer {
    fn reduce(&self, left: Vec<Message>, right: Vec<Message>) -> Vec<Message> {
        add_messages(left, right)
    }
}

/// Core `add_messages` implementation
///
/// Merges message lists with ID-based deduplication and updates.
/// Matches Python `dashflow.graph.message.add_messages` semantics.
///
/// # Algorithm
/// 1. Assign UUIDs to messages without IDs
/// 2. Build index of existing message IDs
/// 3. For each new message:
///    - If ID exists: update existing message
///    - If ID doesn't exist: append to list
///
/// # Example
/// ```
/// use dashflow::core::messages::Message;
/// use dashflow::reducer::add_messages;
///
/// // Append new message
/// let left = vec![Message::human("Hello")];
/// let right = vec![Message::ai("Hi!")];
/// let merged = add_messages(left, right);
/// assert_eq!(merged.len(), 2);
///
/// // Update by ID
/// let mut msg1 = Message::human("Hello");
/// msg1.fields_mut().id = Some("id1".to_string());
///
/// let mut msg1_updated = Message::human("Hello again");
/// msg1_updated.fields_mut().id = Some("id1".to_string());
///
/// let merged = add_messages(vec![msg1], vec![msg1_updated.clone()]);
/// assert_eq!(merged.len(), 1);
/// assert_eq!(merged[0].as_text(), "Hello again");
/// ```
#[must_use]
pub fn add_messages(left: Vec<Message>, right: Vec<Message>) -> Vec<Message> {
    // Step 1: Assign UUIDs to messages without IDs
    let mut left_with_ids = assign_message_ids(left);
    let right_with_ids = assign_message_ids(right);

    // Step 2: Build index of existing message IDs -> position
    let mut id_to_index: std::collections::HashMap<String, usize> = left_with_ids
        .iter()
        .enumerate()
        .filter_map(|(i, msg)| msg.fields().id.as_ref().map(|id| (id.clone(), i)))
        .collect();

    // Step 3: Merge right messages into left
    for right_msg in right_with_ids {
        // Note: assign_message_ids() guarantees all messages have IDs, so this
        // else branch is defensive code that won't execute in normal operation.
        let Some(id) = right_msg.fields().id.as_ref() else {
            left_with_ids.push(right_msg);
            continue;
        };

        // Check if message with this ID already exists
        if let Some(&index) = id_to_index.get(id) {
            // Update existing message
            left_with_ids[index] = right_msg;
        } else {
            // New message - append and track
            id_to_index.insert(id.clone(), left_with_ids.len());
            left_with_ids.push(right_msg);
        }
    }

    left_with_ids
}

/// Assign UUIDs to messages that don't have IDs
fn assign_message_ids(messages: Vec<Message>) -> Vec<Message> {
    messages
        .into_iter()
        .map(|mut msg| {
            if msg.fields().id.is_none() {
                msg.fields_mut().id = Some(Uuid::new_v4().to_string());
            }
            msg
        })
        .collect()
}

/// Helper extension trait for adding IDs to messages (builder pattern)
pub trait MessageExt {
    /// Set the ID field on this message (builder pattern)
    fn with_id(self, id: impl Into<String>) -> Self;
}

impl MessageExt for Message {
    fn with_id(mut self, id: impl Into<String>) -> Self {
        self.fields_mut().id = Some(id.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_messages_append() {
        // New messages without matching IDs should be appended
        let left = vec![Message::human("Hello")];
        let right = vec![Message::ai("Hi there!")];

        let merged = add_messages(left, right);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].as_text(), "Hello");
        assert_eq!(merged[1].as_text(), "Hi there!");
    }

    #[test]
    fn test_add_messages_update_by_id() {
        // Messages with matching IDs should update existing messages
        let mut msg1 = Message::human("Hello");
        msg1.fields_mut().id = Some("msg1".to_string());

        let mut msg1_updated = Message::human("Hello again");
        msg1_updated.fields_mut().id = Some("msg1".to_string());

        let merged = add_messages(vec![msg1], vec![msg1_updated]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].as_text(), "Hello again");
        assert_eq!(merged[0].fields().id.as_deref(), Some("msg1"));
    }

    #[test]
    fn test_add_messages_mixed_operations() {
        // Test both append and update in same merge
        let mut msg1 = Message::human("First");
        msg1.fields_mut().id = Some("id1".to_string());

        let mut msg2 = Message::ai("Second");
        msg2.fields_mut().id = Some("id2".to_string());

        let left = vec![msg1, msg2];

        // Update msg2, add msg3
        let mut msg2_updated = Message::ai("Second updated");
        msg2_updated.fields_mut().id = Some("id2".to_string());

        let msg3 = Message::human("Third");

        let right = vec![msg2_updated, msg3];

        let merged = add_messages(left, right);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].as_text(), "First");
        assert_eq!(merged[1].as_text(), "Second updated");
        assert_eq!(merged[2].as_text(), "Third");
    }

    #[test]
    fn test_add_messages_assigns_ids() {
        // Messages without IDs should get UUIDs assigned
        let left = vec![Message::human("Hello")];
        let right = vec![Message::ai("Hi")];

        let merged = add_messages(left, right);

        // All messages should have IDs now
        assert!(merged[0].fields().id.is_some());
        assert!(merged[1].fields().id.is_some());

        // IDs should be different
        assert_ne!(merged[0].fields().id, merged[1].fields().id);
    }

    #[test]
    fn test_add_messages_preserves_existing_ids() {
        // Messages with IDs should keep their IDs
        let mut msg1 = Message::human("Test");
        msg1.fields_mut().id = Some("custom-id".to_string());

        let merged = add_messages(vec![msg1], vec![]);
        assert_eq!(merged[0].fields().id.as_deref(), Some("custom-id"));
    }

    #[test]
    fn test_add_messages_empty_lists() {
        // Handle empty lists gracefully
        let merged = add_messages(vec![], vec![]);
        assert_eq!(merged.len(), 0);

        let msg = Message::human("Test");
        let merged = add_messages(vec![], vec![msg]);
        assert_eq!(merged.len(), 1);

        let msg = Message::human("Test");
        let merged = add_messages(vec![msg], vec![]);
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn test_message_with_id_builder() {
        let msg = Message::human("Test").with_id("custom-id");
        assert_eq!(msg.fields().id.as_deref(), Some("custom-id"));
    }

    #[test]
    fn test_reducer_trait() {
        let reducer = AddMessagesReducer;
        let left = vec![Message::human("Hello")];
        let right = vec![Message::ai("Hi")];

        let merged = reducer.reduce(left, right);
        assert_eq!(merged.len(), 2);
    }

    // NOTE: GraphState derive macro tests are in tests/graph_state_tests.rs
    // (integration tests) because the macro can't reference dashflow
    // from within the crate's own unit tests
}
