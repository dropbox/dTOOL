# Golden Path Guide - The ONE Recommended Way

**Last Updated:** 2026-01-04 (Worker #2450 - Metadata sync)

**Status:** Official API guidance for DashFlow
**Audience:** All users (beginners to advanced)
**Purpose:** Document the ONE recommended way to do each operation

This guide shows the modern, type-safe, Python-compatible API patterns.

---

## Philosophy

**One Best Way:** For each operation, there is ONE recommended pattern. This reduces cognitive load and ensures consistency across codebases.

**Type Safety:** Modern APIs use `Arc<dyn Tool>` instead of `serde_json::Value` for better compile-time guarantees.

**Python Compatibility:** APIs match Python DashFlow naming and behavior for seamless migration.

**DashFlow Integration:** Agent patterns use DashFlow for advanced features (checkpointing, streaming, human-in-the-loop).

---

## 1. Creating Tools

**Golden Path: Use the `#[tool]` macro**

```rust
use dashflow::core::tool;

#[tool]
/// Multiply two numbers together
fn multiply(
    #[arg(description = "The first number")] a: i64,
    #[arg(description = "The second number")] b: i64,
) -> Result<i64, Box<dyn std::error::Error>> {
    Ok(a * b)
}

// Tool is automatically registered and can be used
let tool = multiply::tool();
```

**Why this pattern:**
- Automatic JSON schema generation from function signature
- Type-safe parameter validation
- Docstring becomes tool description
- Zero boilerplate

**Alternative patterns (not recommended):**
- Manual `Tool` trait implementation (verbose, error-prone)
- JSON schema by hand (hard to maintain)

---

## 2. Binding Tools to Models

**Golden Path: Use `bind_tools()` from `ChatModelToolBindingExt`**

```rust
use dashflow::core::language_models::ChatModelToolBindingExt;
use dashflow_openai::{ChatOpenAI, OpenAIConfig};
use std::sync::Arc;

let config = OpenAIConfig::from_env();
let model = ChatOpenAI::with_config(config)
    .bind_tools(vec![Arc::new(multiply::tool()), Arc::new(add::tool())], None)?;
```

**Why this pattern:**
- Type-safe: accepts `Arc<dyn Tool>`, not raw JSON
- Works across ALL providers (OpenAI, Anthropic, Mistral, etc.)
- Consistent API surface
- Catches errors at compile time

**Deprecated pattern (avoid):**
```rust
// ❌ Old way - deprecated in v1.9.0
model.with_tools(vec![tool_json_1, tool_json_2])
```

---

## 3. Creating Agents

**Golden Path: Use `create_react_agent()` from DashFlow**

```rust
use dashflow::prebuilt::{create_react_agent, AgentState};
use dashflow::core::language_models::ChatModelToolBindingExt;

// 1. Bind tools to model
let model_with_tools = model.bind_tools(tools.clone(), None)?;

// 2. Create agent (one line!)
let agent = create_react_agent(model_with_tools, tools)?;

// 3. Run agent
let initial_state = AgentState::with_human_message("What is 123 * 456?");
let result = agent.invoke(initial_state, None).await?;
```

**Why this pattern:**
- Python-compatible API
- Built-in ReAct reasoning loop
- Integrates with DashFlow features (checkpointing, streaming, interrupt)
- Clean, functional design

**Deprecated pattern (avoid):**
```rust
// ❌ Old way - deprecated in v1.9.0
use dashflow::core::agents::{AgentExecutor, AgentExecutorConfig};

let config = AgentExecutorConfig {
    max_iterations: 10,
    ..Default::default()
};

let agent = AgentExecutor {
    agent: Box::new(my_agent),
    tools: tools,
    config,
};

let result = agent.run(input).await?;
```

---

## 4. State Management

**Golden Path: Use `GraphState` derive macro**

```rust
use dashflow::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(GraphState, Debug, Clone, Serialize, Deserialize)]
struct MyState {
    #[reducer(append)]
    messages: Vec<BaseMessage>,

    #[reducer(default)]
    user_info: String,

    #[reducer(default)]
    iteration: u32,
}
```

**Why this pattern:**
- Automatic reducer generation
- Type-safe state updates
- Clear annotations show merge behavior
- Compile-time validation

**Reducer strategies:**
- `#[reducer(append)]` - Append to Vec
- `#[reducer(default)]` - Replace with new value
- `#[reducer(custom = "fn_name")]` - Custom merge logic

---

## 5. Building Graphs

**Golden Path: Use `StateGraph::new()` with builder pattern**

```rust
use dashflow::prelude::*;

let mut graph = StateGraph::<MyState>::new()?;

// Add nodes
graph.add_node("agent", |state: MyState| async move {
    // Node logic
    Ok(state)
})?;

graph.add_node("tools", tool_node)?;

// Add edges
graph.add_edge(START, "agent")?;
graph.add_conditional_edge("agent", should_continue, vec!["tools", END])?;
graph.add_edge("tools", "agent")?;

// Compile
let app = graph.compile()?;
```

**Why this pattern:**
- Type-safe node definitions
- Clear control flow
- Python-compatible API
- Compile-time validation

---

## 6. Logging and Observability

**Golden Path: Use `DashStreamCallback` for telemetry**

```rust
use dashflow::core::callbacks::DashStreamCallback;
use dashflow::core::language_models::ChatModelOptions;

let callback = DashStreamCallback::new();
let options = ChatModelOptions {
    callbacks: vec![Box::new(callback.clone())],
    ..Default::default()
};

let response = model.invoke_with_options(messages, options).await?;

// Access structured telemetry
for event in callback.events() {
    println!("{:?}", event);
}
```

**Why this pattern:**
- Structured logging (not string output)
- High performance (zero-copy protocol buffers)
- Python-compatible event format
- Production-ready observability

**Features:**
- Token usage tracking
- Latency measurement
- Error reporting
- Tool call telemetry

---

## 7. Streaming Responses

**Golden Path: Use `stream_events()` for unified streaming**

```rust
use dashflow::core::language_models::LLMStream;
use futures::StreamExt;

let mut stream = model.stream_events(messages).await?;

while let Some(event) = stream.next().await {
    match event? {
        StreamEvent::OnChatModelStream { chunk } => {
            print!("{}", chunk.content);
        }
        StreamEvent::OnToolStart { tool, input } => {
            println!("\nCalling tool: {}", tool);
        }
        _ => {}
    }
}
```

**Why this pattern:**
- Unified API for all event types
- Token-by-token streaming
- Tool call streaming
- Agent thought streaming

---

## 8. Composition with Pipe Operator

**Golden Path: Use `|` operator for chaining**

```rust
use dashflow::core::runnable::Runnable;

let chain = prompt | model | output_parser;

let result = chain.invoke(input).await?;
```

**Why this pattern:**
- Functional composition (like Python)
- Type-safe chaining
- Clean, readable code
- Matches Python DashFlow API

**Composable types:**
- ChatPromptTemplate
- ChatModel
- OutputParser
- Tools
- Graphs

---

## 9. Structured Output

**Golden Path: Use `with_structured_output<T>()`**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Person {
    name: String,
    age: u32,
    email: String,
}

let model_with_schema = model.with_structured_output::<Person>()?;

let result: Person = model_with_schema.invoke("Extract: John is 30 years old, email john@example.com").await?;
```

**Why this pattern:**
- Type-safe deserialization
- Automatic JSON schema generation
- Compile-time validation
- Python-compatible API

---

## 10. Human-in-the-Loop

**Golden Path: Use graph interrupt + resume**

```rust
use dashflow::prelude::*;

// 1. Add interrupt before node
graph.add_node("human_review", human_review_node)?;
graph.add_edge("agent", "human_review")?;

// 2. Compile with checkpointer
let checkpointer = MemorySaver::default();
let app = graph.compile_with_checkpointer(Arc::new(checkpointer))?;

// 3. Run until interrupt
let config = RunnableConfig::default().with_thread_id("thread-1");
let result = app.invoke(initial_state, Some(config.clone())).await?;

// 4. Resume after human approval
let updated_state = result.with_approval(true);
let final_result = app.invoke(updated_state, Some(config)).await?;
```

**Why this pattern:**
- Checkpoint-based persistence
- Resume from any point
- Production-ready approval flows
- Python-compatible API

---

## Quick Reference

| Operation | Modern API | Deprecated API |
|-----------|-----------|----------------|
| Create tool | `#[tool] fn name()` | Manual `Tool` impl |
| Bind tools | `bind_tools(vec![Arc::new(tool)])` | `with_tools(vec![json])` |
| Create agent | `create_react_agent(model, tools)` | `AgentExecutor::new()` |
| State management | `#[derive(GraphState)]` | Manual state merging |
| Build graph | `StateGraph::new()` | N/A |
| Logging | `DashStreamCallback::new()` | Print statements |
| Streaming | `stream_events()` | `stream()` |
| Composition | `a \| b \| c` | Manual chaining |
| Structured output | `with_structured_output::<T>()` | Manual parsing |
| Human approval | Graph interrupt + checkpointer | Manual state saving |

---

## Migration Guide

If you're using deprecated APIs:
- Deprecated methods emit warnings via clippy.toml
- Check `clippy.toml` at the repo root for current deprecation guidance
- Follow the patterns in this guide for modern alternatives

Deprecated APIs will be removed in v2.0.0.

---

## Getting Help

**Documentation:**
- This guide (golden path patterns)
- API reference: `cargo doc --open` or docs.rs
- Architecture: docs/ARCHITECTURE.md
- Examples: crates/*/examples/

**Resources:**
- GitHub Issues: Report bugs or feature requests
- Examples: All examples use golden path patterns

---

**Version:** 1.11.3

© 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
