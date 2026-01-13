# dashflow-macros

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](../../LICENSE)

Procedural macros for DashFlow state management in Rust - declarative state reducers with upstream DashFlow (Python) semantics.

## Overview

The `dashflow-macros` crate provides procedural macros for DashFlow state management. The primary macro, `GraphState`, enables declarative state merging with field-level reducer specifications, matching the semantics of upstream DashFlow (Python)'s state reducers.

### Key Features

- **Declarative State Reducers**: Specify field-level merge behavior with attributes
- **Message List Merging**: Built-in `#[add_messages]` reducer for ID-based message deduplication
- **Custom Reducers**: Support for user-defined reducer functions with `#[reducer(fn_name)]`
- **upstream DashFlow (Python) Compatibility**: Matches upstream DashFlow (Python)'s reducer semantics
- **Zero Runtime Cost**: All code generation happens at compile time
- **Type-Safe**: Leverages Rust's type system for safety guarantees

## What is a State Reducer?

In DashFlow, state reducers define how to merge partial state updates into existing state. This is essential for:

- **Agent Conversations**: Merging new messages into conversation history without duplication
- **Accumulation**: Combining values from multiple nodes (e.g., concatenating logs)
- **Updates**: Replacing old values with new ones (default behavior)

The `GraphState` macro generates a `merge_partial` method that applies the appropriate reducer for each field based on its attributes.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
dashflow-macros = "1.11"
dashflow = "1.11"
```

## Quick Start

```rust
use dashflow::GraphStateDerive;
use dashflow::core::messages::Message;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, GraphStateDerive)]
struct AgentState {
    #[add_messages]
    messages: Vec<Message>,

    counter: i32,
}

fn main() {
    let state1 = AgentState {
        messages: vec![Message::human("Hello")],
        counter: 5,
    };

    let state2 = AgentState {
        messages: vec![Message::ai("Hi there!")],
        counter: 10,
    };

    // Merge states: messages append, counter replaces
    let merged = state1.merge_partial(&state2);

    assert_eq!(merged.messages.len(), 2);  // Both messages present
    assert_eq!(merged.counter, 10);         // Latest value wins
}
```

## The `GraphState` Derive Macro

The `GraphState` macro generates a `merge_partial` method that intelligently merges state based on field attributes.

### Generated Method Signature

```rust
impl YourState {
    pub fn merge_partial(&self, partial: &Self) -> Self {
        // Generated merge logic
    }
}
```

### Field Attributes

#### `#[add_messages]` - Message List Reducer

The `#[add_messages]` attribute provides ID-based message list merging with upstream DashFlow (Python) semantics:

- **Append-only by default**: New messages are added to the list
- **ID-based updates**: Messages with matching IDs replace existing messages
- **Automatic UUID assignment**: Messages without IDs receive UUIDs

```rust
use dashflow::GraphStateDerive;
use dashflow::core::messages::Message;
use dashflow::reducer::MessageExt;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, GraphStateDerive)]
struct AgentState {
    #[add_messages]
    messages: Vec<Message>,
}

fn main() {
    let state1 = AgentState {
        messages: vec![
            Message::human("Hello").with_id("msg1"),
            Message::ai("Hi").with_id("msg2"),
        ],
    };

    let state2 = AgentState {
        messages: vec![
            Message::ai("Hi! How can I help?").with_id("msg2"), // Update msg2
            Message::human("Tell me about Rust").with_id("msg3"), // Append msg3
        ],
    };

    let merged = state1.merge_partial(&state2);

    // Result: 3 messages (msg1, updated msg2, msg3)
    assert_eq!(merged.messages.len(), 3);
    assert_eq!(merged.messages[1].as_text(), "Hi! How can I help?");
}
```

#### `#[reducer(fn_name)]` - Custom Reducer Function

The `#[reducer(fn_name)]` attribute allows you to specify a custom reducer function for a field:

```rust
use dashflow::GraphStateDerive;
use serde::{Deserialize, Serialize};

fn concat_logs(left: String, right: String) -> String {
    if left.is_empty() {
        right
    } else {
        format!("{}\n{}", left, right)
    }
}

#[derive(Clone, Serialize, Deserialize, GraphStateDerive)]
struct AgentState {
    #[reducer(concat_logs)]
    log: String,

    #[reducer(|a, b| a + b)]  // Inline closures not supported yet
    counter: i32,
}

fn add_counters(a: i32, b: i32) -> i32 {
    a + b
}

#[derive(Clone, Serialize, Deserialize, GraphStateDerive)]
struct CounterState {
    #[reducer(add_counters)]
    counter: i32,
}
```

**Note**: Reducer functions must have the signature `fn(T, T) -> T` where `T` is the field type.

#### No Attribute - Right Side Wins (Default)

Fields without attributes use the "right side wins" strategy - the value from the partial update replaces the existing value:

```rust
use dashflow::GraphStateDerive;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, GraphStateDerive)]
struct AgentState {
    value: i32,      // No attribute: right side wins
    text: String,    // No attribute: right side wins
}

fn main() {
    let state1 = AgentState {
        value: 5,
        text: "first".to_string(),
    };

    let state2 = AgentState {
        value: 10,
        text: "second".to_string(),
    };

    let merged = state1.merge_partial(&state2);

    assert_eq!(merged.value, 10);           // state2 value
    assert_eq!(merged.text, "second");      // state2 text
}
```

## Usage Examples

### Example 1: Agent with Message History

```rust
use dashflow::GraphStateDerive;
use dashflow::core::messages::Message;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, GraphStateDerive)]
struct ConversationState {
    #[add_messages]
    messages: Vec<Message>,
}

fn main() {
    let mut state = ConversationState {
        messages: vec![Message::human("What's the weather?")],
    };

    // Agent responds
    let response = ConversationState {
        messages: vec![Message::ai("Let me check that for you")],
    };
    state = state.merge_partial(&response);

    // Tool returns result
    let tool_result = ConversationState {
        messages: vec![Message::tool("Sunny, 72°F", "call_123")],
    };
    state = state.merge_partial(&tool_result);

    assert_eq!(state.messages.len(), 3);
}
```

### Example 2: Multi-Field State with Mixed Reducers

```rust
use dashflow::GraphStateDerive;
use dashflow::core::messages::Message;
use serde::{Deserialize, Serialize};

fn concat_logs(left: String, right: String) -> String {
    if left.is_empty() {
        right
    } else {
        format!("{}\n{}", left, right)
    }
}

fn sum_numbers(left: i32, right: i32) -> i32 {
    left + right
}

#[derive(Clone, Serialize, Deserialize, GraphStateDerive)]
struct AgentState {
    #[add_messages]
    messages: Vec<Message>,

    #[reducer(concat_logs)]
    log: String,

    #[reducer(sum_numbers)]
    total_tokens: i32,

    current_step: String,  // No reducer: right side wins
}

fn main() {
    let state1 = AgentState {
        messages: vec![Message::human("Hello")],
        log: "Agent started".to_string(),
        total_tokens: 100,
        current_step: "thinking".to_string(),
    };

    let state2 = AgentState {
        messages: vec![Message::ai("Hi there!")],
        log: "Agent responded".to_string(),
        total_tokens: 50,
        current_step: "complete".to_string(),
    };

    let merged = state1.merge_partial(&state2);

    assert_eq!(merged.messages.len(), 2);
    assert_eq!(merged.log, "Agent started\nAgent responded");
    assert_eq!(merged.total_tokens, 150);
    assert_eq!(merged.current_step, "complete");
}
```

### Example 3: ID-Based Message Updates

```rust
use dashflow::GraphStateDerive;
use dashflow::core::messages::Message;
use dashflow::reducer::MessageExt;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, GraphStateDerive)]
struct AgentState {
    #[add_messages]
    messages: Vec<Message>,
}

fn main() {
    // Initial state with thinking message
    let state = AgentState {
        messages: vec![
            Message::human("What's 2+2?"),
            Message::ai("Let me calculate...").with_id("thinking"),
        ],
    };

    // Update the thinking message with final answer (same ID)
    let update = AgentState {
        messages: vec![
            Message::ai("2+2 equals 4!").with_id("thinking"),
        ],
    };

    let merged = state.merge_partial(&update);

    // Still 2 messages, but the AI message is updated
    assert_eq!(merged.messages.len(), 2);
    assert_eq!(merged.messages[1].as_text(), "2+2 equals 4!");
}
```

### Example 4: Custom Accumulation Reducer

```rust
use dashflow::GraphStateDerive;
use serde::{Deserialize, Serialize};

fn merge_lists(left: Vec<String>, right: Vec<String>) -> Vec<String> {
    let mut result = left;
    result.extend(right);
    result
}

#[derive(Clone, Serialize, Deserialize, GraphStateDerive)]
struct SearchState {
    #[reducer(merge_lists)]
    results: Vec<String>,

    query: String,  // Right side wins
}

fn main() {
    let state1 = SearchState {
        results: vec!["Result 1".to_string(), "Result 2".to_string()],
        query: "rust tutorials".to_string(),
    };

    let state2 = SearchState {
        results: vec!["Result 3".to_string()],
        query: "rust tutorials".to_string(),
    };

    let merged = state1.merge_partial(&state2);

    assert_eq!(merged.results.len(), 3);  // All results accumulated
}
```

## Requirements

### Must Derive `Clone`

The `GraphState` macro requires that your state struct implements `Clone` because the generated `merge_partial` method clones field values:

```rust
#[derive(Clone, GraphStateDerive)]  // ✅ Clone required
struct MyState {
    value: i32,
}
```

### Named Fields Only

The macro only works with structs that have named fields:

```rust
// ✅ Supported
#[derive(GraphStateDerive)]
struct MyState {
    field1: i32,
    field2: String,
}

// ❌ Not supported (tuple struct)
#[derive(GraphStateDerive)]
struct MyState(i32, String);
```

### Reducer Function Signature

Custom reducer functions must have the signature `fn(T, T) -> T`:

```rust
// ✅ Valid reducer
fn add_numbers(left: i32, right: i32) -> i32 {
    left + right
}

// ❌ Invalid (wrong signature)
fn invalid_reducer(a: i32) -> i32 {
    a * 2
}
```

## Generated Code

For this struct:

```rust
#[derive(Clone, GraphStateDerive)]
struct AgentState {
    #[add_messages]
    messages: Vec<Message>,

    #[reducer(sum_values)]
    counter: i32,

    current_step: String,
}

fn sum_values(a: i32, b: i32) -> i32 {
    a + b
}
```

The macro generates:

```rust
impl AgentState {
    pub fn merge_partial(&self, partial: &Self) -> Self {
        Self {
            messages: ::dashflow::__private::reducer::add_messages(
                self.messages.clone(),
                partial.messages.clone()
            ),
            counter: sum_values(
                self.counter.clone(),
                partial.counter.clone()
            ),
            current_step: partial.current_step.clone(),
        }
    }
}
```

## upstream DashFlow (Python) Compatibility

This macro is designed to match upstream DashFlow (Python)'s state reducer semantics:

| upstream DashFlow (Python) | Rust Equivalent |
|-----------------|-----------------|
| `add_messages` reducer | `#[add_messages]` attribute |
| Custom reducer annotation | `#[reducer(fn_name)]` attribute |
| Default (right wins) | No attribute |
| Message ID updates | Same ID-based update logic |
| Automatic IDs | Same UUID assignment |

**Python Example:**

```python
from dashflow.graph import add_messages
from typing import Annotated

class AgentState:
    messages: Annotated[list[Message], add_messages]
    counter: int
```

**Rust Equivalent:**

```rust
#[derive(GraphStateDerive)]
struct AgentState {
    #[add_messages]
    messages: Vec<Message>,
    counter: i32,
}
```

## Running Examples

The `dashflow` crate includes comprehensive examples demonstrating the macro:

```bash
# State reducers example
cargo run --example state_reducers --package dashflow

# Distributed checkpointing with state
cargo run --example distributed_checkpointing --package dashflow
```

## Testing

The macro is extensively tested in the `dashflow` crate:

```bash
# Run integration tests
cargo test --test graph_state_tests --package dashflow

# Run property-based tests
cargo test --test property_tests --package dashflow
```

## Debugging Macro Expansion

To see the generated code, use `cargo expand`:

```bash
# Install cargo-expand
cargo install cargo-expand

# Expand a specific test
cargo expand --test graph_state_tests --package dashflow
```

## Implementation Details

### Macro Process

1. **Parse Input**: Extract struct name, fields, and attributes
2. **Validate**: Ensure struct has named fields and Clone trait
3. **Generate Field Merges**: For each field, generate merge logic based on attributes
4. **Emit Code**: Generate `merge_partial` method implementation

### Error Messages

The macro provides clear error messages for common issues:

```rust
// ❌ Error: GraphState can only be derived for structs with named fields
#[derive(GraphStateDerive)]
struct MyState(i32);

// ❌ Error: GraphState can only be derived for structs
#[derive(GraphStateDerive)]
enum MyEnum {
    Variant1,
}
```

### Internal Implementation

The macro uses `syn` for parsing, `quote` for code generation, and `proc-macro2` for token manipulation. It generates calls to:

- `::dashflow::__private::reducer::add_messages` for message merging
- User-provided functions for custom reducers
- Direct field access for default (right-wins) behavior

## Performance

- **Zero runtime overhead**: All code generation happens at compile time
- **No reflection**: Generated code is direct field access and function calls
- **Inlining**: Generated methods are eligible for inlining
- **Type safety**: All type checking happens at compile time

## Common Patterns

### Pattern 1: Agent State with Multiple Concerns

```rust
#[derive(Clone, Serialize, Deserialize, GraphStateDerive)]
struct AgentState {
    #[add_messages]
    messages: Vec<Message>,

    #[reducer(concat_logs)]
    debug_log: String,

    #[reducer(sum_tokens)]
    total_tokens: i32,

    current_tool: Option<String>,  // Latest tool
    is_complete: bool,              // Latest status
}
```

### Pattern 2: Workflow State with Accumulation

```rust
#[derive(Clone, Serialize, Deserialize, GraphStateDerive)]
struct WorkflowState {
    #[reducer(merge_results)]
    results: Vec<SearchResult>,

    #[reducer(|a, b| a.max(b))]
    confidence_score: f64,

    current_stage: String,
}
```

### Pattern 3: Streaming Updates

```rust
#[derive(Clone, Serialize, Deserialize, GraphStateDerive)]
struct StreamState {
    #[add_messages]
    messages: Vec<Message>,

    #[reducer(append_chunks)]
    generated_text: String,

    is_streaming: bool,
}
```

## Limitations

- **No inline closures**: `#[reducer(|a, b| a + b)]` is not supported - use named functions
- **No generic reducer functions**: Reducer functions must have concrete types
- **No async reducers**: Reducer functions must be synchronous
- **Clone requirement**: All fields must implement `Clone`

## Related Crates

- **dashflow**: Main DashFlow implementation that uses these macros
- **dashflow::core**: Core message types and traits
- **syn**: Parser for Rust syntax used by the macro
- **quote**: Code generation utilities

## Contributing

Contributions are welcome! Please see the main [DashFlow repository](https://github.com/dropbox/dTOOL/dashflow) for contribution guidelines.

When contributing to this crate:
- Add tests to `dashflow/tests/graph_state_tests.rs`
- Update examples in `dashflow/examples/`
- Document new attributes or features

## License

This crate is part of the DashFlow project and is licensed under the MIT License. See the [LICENSE](../../LICENSE) file for details.

## Version

Current version: **1.11**

## Support

- **Documentation**: See source code and this README (crate not published to crates.io)
- **Issues**: [GitHub Issues](https://github.com/dropbox/dTOOL/dashflow/issues)
- **Discussions**: [GitHub Discussions](https://github.com/dropbox/dTOOL/dashflow/discussions)

## See Also

- [DashFlow Python Documentation](https://dashflow-ai.github.io/dashflow/) (upstream reference)
- [State Reducers in DashFlow Python](https://dashflow-ai.github.io/dashflow/reference/graphs/#add_messages)
- [Rust Procedural Macros Book](https://doc.rust-lang.org/reference/procedural-macros.html)
