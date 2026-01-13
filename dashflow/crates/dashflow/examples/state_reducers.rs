//! Example demonstrating state reducers with the GraphState derive macro
//!
//! This example shows how to use the `add_messages` reducer for automatic
//! message list merging with ID-based deduplication, matching Python DashFlow
//! semantics.
//!
//! Run with:
//! ```bash
//! cargo run --example state_reducers
//! ```

use dashflow::core::messages::Message;
use dashflow::reducer::{add_messages, MessageExt};
use dashflow::GraphStateDerive;
use serde::{Deserialize, Serialize};

/// Example 1: Manual usage of add_messages function
fn example_manual_add_messages() {
    println!("=== Example 1: Manual add_messages ===\n");

    // Initial messages
    let left = vec![
        Message::human("Hello").with_id("msg1"),
        Message::ai("Hi there!").with_id("msg2"),
    ];

    // New messages - one update, one append
    let right = vec![
        Message::ai("Hi there! How can I help?").with_id("msg2"), // Update msg2
        Message::human("Tell me about Rust").with_id("msg3"),     // Append msg3
    ];

    let merged = add_messages(left, right);

    println!("Result: {} messages", merged.len());
    for (i, msg) in merged.iter().enumerate() {
        println!(
            "  [{}] {}: {} (id: {:?})",
            i,
            msg.message_type(),
            msg.as_text(),
            msg.fields().id
        );
    }
    println!();
}

/// Example 2: Using GraphState derive macro
fn example_graph_state_macro() {
    println!("=== Example 2: GraphState Derive Macro ===\n");

    fn concat_logs(left: String, right: String) -> String {
        if left.is_empty() {
            right
        } else if right.is_empty() {
            left
        } else {
            format!("{}\n{}", left, right)
        }
    }

    #[derive(Clone, Serialize, Deserialize, GraphStateDerive)]
    struct AgentState {
        #[add_messages]
        messages: Vec<Message>,

        #[reducer(concat_logs)]
        log: String,

        counter: i32,
    }

    let state1 = AgentState {
        messages: vec![Message::human("Hello").with_id("msg1")],
        log: "Started agent".to_string(),
        counter: 1,
    };

    let state2 = AgentState {
        messages: vec![
            Message::ai("Hi!").with_id("msg2"),
            Message::human("Updated hello").with_id("msg1"), // Update msg1
        ],
        log: "Agent responded".to_string(),
        counter: 5,
    };

    let merged = state1.merge_partial(&state2);

    println!("Messages ({}):", merged.messages.len());
    for msg in &merged.messages {
        println!("  - {}: {}", msg.message_type(), msg.as_text());
    }
    println!("\nLog:\n{}", merged.log);
    println!("\nCounter: {}", merged.counter);
    println!("(Counter uses right-side value since no reducer specified)\n");
}

/// Example 3: ID-based update semantics
fn example_id_based_updates() {
    println!("=== Example 3: ID-based Updates ===\n");

    // Start with 3 messages
    let messages = vec![
        Message::human("What's 2+2?").with_id("q1"),
        Message::ai("Let me calculate...").with_id("a1"),
        Message::human("And 3+3?").with_id("q2"),
    ];

    println!("Initial messages ({}): ", messages.len());
    for msg in &messages {
        let id = msg.fields().id.as_deref().unwrap_or("<no-id>");
        println!("  {id}: {}", msg.as_text());
    }

    // Update the AI response (same ID)
    let updates = vec![Message::ai("2+2 equals 4!").with_id("a1")];

    let merged = add_messages(messages, updates);

    println!("\nAfter update ({}): ", merged.len());
    for msg in &merged {
        let id = msg.fields().id.as_deref().unwrap_or("<no-id>");
        println!("  {id}: {}", msg.as_text());
    }
    println!();
}

/// Example 4: Automatic ID assignment
fn example_auto_id_assignment() {
    println!("=== Example 4: Automatic ID Assignment ===\n");

    // Messages without IDs
    let left = vec![Message::human("Hello"), Message::ai("Hi")];

    let right = vec![Message::human("How are you?")];

    let merged = add_messages(left, right);

    println!("All messages automatically receive UUIDs:");
    for msg in &merged {
        let id = msg.fields().id.as_deref().unwrap_or("<no-id>");
        println!(
            "  {}: {} (id: {})",
            msg.message_type(),
            msg.as_text(),
            id
        );
    }
    println!();
}

/// Example 5: Real-world agent scenario
fn example_agent_scenario() {
    println!("=== Example 5: Agent Message History ===\n");

    #[derive(Clone, Serialize, Deserialize, GraphStateDerive)]
    struct AgentState {
        #[add_messages]
        messages: Vec<Message>,
    }

    // Simulate an agent conversation
    let mut state = AgentState {
        messages: vec![Message::human("Search for Rust tutorials")],
    };

    println!("Turn 1: Human asks question");
    println!("  Messages: {}", state.messages.len());

    // Agent thinks and calls a tool
    let agent_update = AgentState {
        messages: vec![Message::ai("I'll search for that").with_id("think1")],
    };
    state = state.merge_partial(&agent_update);

    println!("\nTurn 2: Agent responds with tool call");
    println!("  Messages: {}", state.messages.len());

    // Tool returns result
    let tool_update = AgentState {
        messages: vec![Message::tool("Found 10 tutorials", "call_1")],
    };
    state = state.merge_partial(&tool_update);

    println!("\nTurn 3: Tool returns results");
    println!("  Messages: {}", state.messages.len());

    // Agent provides final answer (updating its previous thinking message)
    let final_update = AgentState {
        messages: vec![
            Message::ai("I found 10 great Rust tutorials for you!").with_id("think1"), // Update
        ],
    };
    state = state.merge_partial(&final_update);

    println!("\nTurn 4: Agent provides final answer");
    println!("  Messages: {}", state.messages.len());

    println!("\nFinal conversation:");
    for (i, msg) in state.messages.iter().enumerate() {
        println!("  [{}] {}: {}", i, msg.message_type(), msg.as_text());
    }
    println!();
}

fn main() {
    println!("\n🦀 DashFlow Rust - State Reducers Example\n");
    println!("This example demonstrates Python DashFlow's add_messages reducer");
    println!("ported to Rust with the same semantics:\n");
    println!("  • Append-only by default (new messages added to list)");
    println!("  • ID-based updates (messages with matching IDs replace existing)");
    println!("  • Automatic UUID assignment (messages without IDs get UUIDs)\n");
    println!("{}", "=".repeat(60));
    println!();

    example_manual_add_messages();
    example_graph_state_macro();
    example_id_based_updates();
    example_auto_id_assignment();
    example_agent_scenario();

    println!("{}", "=".repeat(60));
    println!("\n✅ All examples completed!");
    println!("\n💡 Key Takeaways:");
    println!("  1. Use add_messages() directly for manual merging");
    println!("  2. Use #[derive(GraphState)] for declarative state management");
    println!("  3. Messages with same ID update (don't duplicate)");
    println!("  4. Messages without IDs get automatic UUIDs");
    println!("  5. Custom reducers supported with #[reducer(fn_name)]\n");
}
