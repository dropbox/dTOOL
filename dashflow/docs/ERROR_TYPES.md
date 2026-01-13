# DashFlow Error Types Reference

**Last Updated:** 2026-01-02 (Worker #2337 - Add missing Last Updated headers)

This document describes the error types used throughout DashFlow, their categories, and how to handle them.

## Table of Contents

- [Error Design Philosophy](#error-design-philosophy)
- [Main Error Types](#main-error-types)
- [Error Categories](#error-categories)
- [Actionable Errors](#actionable-errors)
- [Checkpoint Errors](#checkpoint-errors)
- [Streaming Errors](#streaming-errors)
- [Crate-Specific Errors](#crate-specific-errors)
- [Error Handling Patterns](#error-handling-patterns)

## Error Design Philosophy

DashFlow errors are designed with these principles:

1. **Actionable**: Errors tell you what went wrong, why it's a problem, and how to fix it
2. **Categorized**: Errors are classified by root cause (billing, auth, code bug, network, etc.)
3. **Typed**: Use Rust's type system for pattern matching and explicit handling
4. **AI-friendly**: Error messages include code snippets for automated debugging

## Main Error Types

### Graph Error (`dashflow::error::Error`)

The primary error type for graph execution operations.

```rust
use dashflow::error::{Error, Result};

fn build_graph() -> Result<CompiledGraph<MyState>> {
    let mut graph = StateGraph::new();
    graph.add_node("start", start_node);
    graph.set_entry_point("start");  // Required!
    graph.compile()
}
```

**Variants:**

| Variant | Description | Recovery |
|---------|-------------|----------|
| `Validation(String)` | Graph structure is invalid | Fix graph definition |
| `NodeExecution { node, source }` | Node failed during execution | Check node implementation |
| `NoEntryPoint` | Graph has no entry point | Call `set_entry_point()` |
| `NodeNotFound(String)` | Referenced node doesn't exist | Add the node or fix the name |
| `DuplicateNodeName(String)` | Node name already exists | Use unique name or `add_node_or_replace()` |
| `CycleDetected(String)` | Unexpected cycle in graph | Use `allow_cycles()` or fix edges |
| `InvalidEdge(String)` | Edge references missing nodes | Add nodes before edges |
| `Timeout(Duration)` | Execution exceeded time limit | Increase timeout or optimize |
| `Serialization(serde_json::Error)` | State serialization failed | Check `Serialize` derives |
| `RecursionLimit { limit }` | Too many graph steps | Check for infinite loops |
| `StateSizeExceeded { node, actual_bytes, max_bytes }` | State grew too large | Reduce state or increase limit |
| `InterruptWithoutCheckpointer(String)` | Interrupt needs persistence | Add checkpointer |
| `ResumeWithoutCheckpointer` | Resume needs persistence | Add checkpointer |
| `NoCheckpointToResume(String)` | No checkpoint for thread | Start fresh or check thread_id |

### Core Error (`dashflow::core::Error`)

Low-level errors from core operations (API calls, I/O, etc.).

```rust
use dashflow::core::{Error, ErrorCategory};

fn handle_api_error(err: Error) {
    match err.category() {
        ErrorCategory::AccountBilling => {
            eprintln!("Add credits to your account");
        }
        ErrorCategory::Authentication => {
            eprintln!("Check your API key");
        }
        ErrorCategory::Network => {
            eprintln!("Retrying...");
        }
        ErrorCategory::CodeBug => {
            panic!("Bug in code: {}", err);
        }
        _ => eprintln!("Error: {}", err),
    }
}
```

**Variants:**

| Variant | Category | Description |
|---------|----------|-------------|
| `AccountBilling(String)` | AccountBilling | Insufficient credits, quota exceeded |
| `Authentication(String)` | Authentication | Invalid API key, expired token |
| `InvalidInput(String)` | Validation | Bad user input |
| `Configuration(String)` | Validation | Invalid configuration |
| `Serialization(serde_json::Error)` | Unknown | JSON parsing failed |
| `Io(std::io::Error)` | Unknown | File/network I/O failed |
| `Network(String)` | Network | Connection issues |
| `Http(String)` | Unknown | HTTP request failed |
| `Api(String)` | Varies* | LLM API error |
| `ApiFormat(String)` | ApiFormat | Unexpected API response format |
| `RateLimit(String)` | Network | Too many requests |
| `Timeout(String)` | Network | Operation timed out |
| `ToolExecution(String)` | CodeBug | Tool implementation failed |
| `RunnableExecution(String)` | CodeBug | Runnable implementation failed |
| `OutputParsing(String)` | Unknown | Failed to parse LLM output |
| `Agent(String)` | Unknown | Agent execution error |

*`Api` errors are auto-categorized based on error message content.

## Error Categories

The `ErrorCategory` enum classifies errors by root cause:

```rust
pub enum ErrorCategory {
    AccountBilling,  // Account needs credits or upgrade
    Authentication,  // Invalid keys or tokens
    CodeBug,         // Logic errors in your code
    ApiFormat,       // API response doesn't match expected format
    Network,         // Connection, timeout, rate limit
    Validation,      // Bad input from user
    Unknown,         // Unclassified
}
```

### Helper Methods

```rust
let err = Error::api("invalid api key");

// Check if this is a code bug that needs fixing
if err.is_code_bug() {
    panic!("Bug: {}", err);
}

// Check if this is environmental (not your code)
if err.is_environmental() {
    eprintln!("External issue: {}", err);
}

// Get formatted status message
eprintln!("{}", err.status_message());
// Output: [Authentication/Authorization Issue] API error: invalid api key
```

## Actionable Errors

Many errors implement `ActionableError` trait, providing fix suggestions:

```rust
use dashflow::error::{Error, ActionableError};

fn handle_error(err: &Error) {
    eprintln!("Error: {}", err);

    if let Some(suggestion) = err.suggestion() {
        eprintln!("\nHow to fix:");
        eprintln!("{}", suggestion.description);

        if let Some(code) = &suggestion.code_snippet {
            eprintln!("\nExample:\n```rust{}\n```", code);
        }
    }
}
```

**Example output:**

```
Error: Graph has no entry point defined

How to fix:
Set an entry point for your graph using set_entry_point()

Example:
```rust
let mut graph = StateGraph::<MyState>::new();
graph.add_node("start", start_node);
graph.add_node("end", end_node);
graph.set_entry_point("start");  // <-- Add this
graph.add_edge("start", "end");
```
```

## Checkpoint Errors

`CheckpointError` provides detailed errors for persistence operations:

```rust
use dashflow::error::CheckpointError;

fn handle_checkpoint_error(err: CheckpointError) {
    // Check recoverability
    if err.is_recoverable() {
        // Transient errors: retry
        eprintln!("Retrying checkpoint operation...");
    } else if err.is_corruption() {
        // Data corruption: alert!
        eprintln!("ALERT: Data corruption detected");
    } else if err.is_configuration_issue() {
        // Config problem: fix setup
        eprintln!("Fix configuration: {}", err);
    }
}
```

**Variants:**

| Variant | Recoverable? | Description |
|---------|--------------|-------------|
| `StorageFull { path, available, required }` | No (config) | Disk full |
| `ConnectionLost { backend, reason }` | Yes | Connection dropped |
| `SerializationFailed { reason }` | No | State can't be serialized |
| `DeserializationFailed { reason }` | No (corruption) | State can't be deserialized |
| `IntegrityCheckFailed { checkpoint_id, reason }` | No (corruption) | Checksum mismatch |
| `NotFound { checkpoint_id }` | No | Checkpoint doesn't exist |
| `PermissionDenied { path, reason }` | No (config) | Access denied |
| `Timeout { duration }` | Yes | Operation timed out |
| `IndexCorrupted { path, reason }` | No (corruption) | Index file corrupted |
| `SchemaMismatch { found, expected }` | No (config) | Version mismatch |
| `MigrationFailed { from, to, reason }` | No | Schema migration failed |
| `QuorumNotAchieved { successes, total, required }` | Yes | Replication failed |
| `LockFailed { path, reason }` | Yes | Lock contention |

## Streaming Errors

`dashflow_streaming::Error` handles streaming-specific errors:

```rust
use dashflow_streaming::Error;

fn handle_streaming_error(err: Error) {
    match err {
        Error::ProtobufEncode(e) => eprintln!("Encoding failed: {}", e),
        Error::ProtobufDecode(e) => eprintln!("Decoding failed: {}", e),
        Error::Compression(msg) => eprintln!("Compression failed: {}", msg),
        Error::Decompression(msg) => eprintln!("Decompression failed: {}", msg),
        Error::InvalidFormat(msg) => eprintln!("Invalid format: {}", msg),
        Error::Io(e) => eprintln!("I/O error: {}", e),
        Error::Json(e) => eprintln!("JSON error: {}", e),
        Error::Serialization(msg) => eprintln!("Serialization: {}", msg),
        Error::DiffError(msg) => eprintln!("Diff/patch: {}", msg),
    }
}
```

## Crate-Specific Errors

Each DashFlow crate defines domain-specific errors:

| Crate | Error Type | Purpose |
|-------|------------|---------|
| `dashflow` | `Error`, `CheckpointError` | Graph execution, checkpointing |
| `dashflow` | `core::Error` | Core operations, API calls |
| `dashflow-streaming` | `Error` | Streaming, compression |
| `dashflow-registry` | `RegistryError` | Package registry |
| `dashflow-langsmith` | `Error` | LangSmith integration |
| `dashflow-langserve` | `LangServeError` | LangServe integration |
| `dashflow-redis-checkpointer` | `RedisCheckpointerError` | Redis storage |
| `dashflow-postgres-checkpointer` | `PostgresError`, `Error` | PostgreSQL storage |
| `dashflow-s3-checkpointer` | `S3CheckpointerError` | S3 storage |
| `dashflow-dynamodb-checkpointer` | `DynamoDBCheckpointerError` | DynamoDB storage |
| `dashflow-memory` | `MemoryError` | Memory stores |
| `dashflow-text-splitters` | `Error` | Text splitting |
| `dashflow-compression` | `CompressionError` | Compression |
| `dashflow-observability` | `Error` | Telemetry |
| `dashflow-wasm-executor` | `Error` | WASM execution |
| `dashflow-git-tool` | `GitError` | Git operations |
| `dashflow-context` | `ContextError` | Context management |
| `dashflow-shell-tool` | `SandboxError` | Sandboxed execution |
| `dashflow-prompts` | `PromptError` | Prompt management |
| `dashflow-clickup` | `ClickupError` | ClickUp integration |
| `dashflow-project` | `ProjectError` | Project discovery |
| `dashflow-remote-node` | `Error` | Remote node execution |

## Error Handling Patterns

### Pattern 1: Match and Handle

```rust
use dashflow::error::Error;

fn execute_graph(graph: CompiledGraph<MyState>) -> Result<MyState, Error> {
    match graph.invoke(initial_state).await {
        Ok(state) => Ok(state),
        Err(Error::RecursionLimit { limit }) => {
            eprintln!("Hit recursion limit of {}. Possible infinite loop.", limit);
            Err(Error::RecursionLimit { limit })
        }
        Err(Error::Timeout(duration)) => {
            eprintln!("Timed out after {:?}. Increasing timeout...", duration);
            // Retry with longer timeout
            graph.with_timeout(duration * 2).invoke(initial_state).await
        }
        Err(e) => Err(e),
    }
}
```

### Pattern 2: Category-Based Handling

```rust
use dashflow::core::{Error, ErrorCategory};

async fn call_llm_with_retry(prompt: &str) -> Result<String, Error> {
    for attempt in 1..=3 {
        match llm.invoke(prompt).await {
            Ok(response) => return Ok(response),
            Err(e) if e.is_environmental() => {
                eprintln!("Attempt {}: {} (retrying)", attempt, e);
                tokio::time::sleep(Duration::from_secs(attempt as u64)).await;
            }
            Err(e) => return Err(e),  // Code bugs should not be retried
        }
    }
    Err(Error::network("Max retries exceeded"))
}
```

### Pattern 3: Actionable Error Reporting

```rust
use dashflow::error::{Error, ActionableError};

fn report_error(err: &Error) {
    // Log full error with fix suggestion
    eprintln!("{}", err.format_with_suggestion());

    // Or manually format for custom output
    if err.has_suggestion() {
        let suggestion = err.suggestion().unwrap();
        log::error!("Error: {}", err);
        log::info!("Fix: {}", suggestion.description);
        if let Some(code) = &suggestion.code_snippet {
            log::debug!("Example code: {}", code);
        }
    }
}
```

### Pattern 4: Converting Between Error Types

```rust
// Core errors convert to graph errors
let core_err: dashflow::core::Error = Error::api("test");
let graph_err: dashflow::error::Error = Error::Core(core_err);

// Checkpoint errors convert to graph errors
let chkpt_err = CheckpointError::NotFound {
    checkpoint_id: "abc".to_string()
};
let graph_err: Error = chkpt_err.into();
```

## Best Practices

1. **Don't swallow errors**: Always log or propagate errors
2. **Use categories**: Check `is_code_bug()` vs `is_environmental()` for appropriate handling
3. **Provide context**: Wrap errors with additional context when propagating
4. **Retry smartly**: Only retry `is_recoverable()` or environmental errors
5. **Surface suggestions**: Use `format_with_suggestion()` for user-facing errors
6. **Type your handlers**: Use pattern matching for explicit handling of each variant

## Creating New Error Types (M-148 Guidelines)

When creating error types for new crates, follow these guidelines to reduce error sprawl and maintain consistency.

### Decision Tree: Do You Need a New Error Type?

```
Does your crate make external API calls or have domain-specific failures?
├── Yes → Create a crate-specific error type
│   └── But also implement `From<YourError> for dashflow::core::Error`
└── No → Just use `dashflow::core::Error` or `anyhow::Error`
```

### Required Elements for Crate Error Types

1. **Use `thiserror` for derive macros**:
   ```rust
   use thiserror::Error;

   #[derive(Debug, Error)]
   #[non_exhaustive]  // Always include for forward compatibility
   pub enum MyError {
       #[error("failed to connect: {0}")]
       Connection(String),
   }
   ```

2. **Implement `From<YourError> for dashflow::Error`** (if your crate depends on `dashflow`):

   **Note:** This requires adding `dashflow = { path = "../dashflow" }` to your Cargo.toml.
   Some utility crates intentionally avoid this dependency to stay lightweight.

   ```rust
   // For checkpointer crates - convert to graph Error with CheckpointError
   impl From<MyError> for dashflow::Error {
       fn from(err: MyError) -> Self {
           use dashflow::error::CheckpointError;
           let checkpoint_err = match err {
               MyError::Connection(msg) => CheckpointError::ConnectionLost {
                   backend: "mybackend".to_string(),
                   reason: msg,
               },
               MyError::NotFound(id) => CheckpointError::NotFound { checkpoint_id: id },
               MyError::Other(msg) => CheckpointError::Other(msg),
           };
           dashflow::Error::Checkpoint(checkpoint_err)
       }
   }
   ```

   **Crates with From implementations:** `dashflow-postgres-checkpointer`, `dashflow-redis-checkpointer`,
   `dashflow-dynamodb-checkpointer`, `dashflow-s3-checkpointer`.

   **Crates without (intentionally standalone):** `dashflow-observability`, `dashflow-langserve`,
   `dashflow-registry`, `dashflow-streaming`.

3. **Map variants to appropriate core error categories**:
   | Your Error Kind | Core Error Constructor | Category |
   |-----------------|------------------------|----------|
   | Connection/Network | `Error::network()` | Network |
   | Auth/Credentials | `Error::authentication()` | Authentication |
   | Billing/Credits | `Error::account_billing()` | AccountBilling |
   | Bad input | `Error::invalid_input()` | Validation |
   | Config issues | `Error::config()` | Validation |
   | Rate limits | `Error::rate_limit()` | Network |
   | Timeouts | `Error::timeout()` | Network |
   | Internal bugs | `Error::other()` | Unknown |

4. **Add a `Result` type alias**:
   ```rust
   pub type Result<T> = std::result::Result<T, MyError>;
   ```

5. **Add tests for Display and Debug**:
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn test_error_display() {
           let err = MyError::Connection("timeout".to_string());
           assert!(err.to_string().contains("timeout"));
       }
   }
   ```

### Checklist for New Error Types

- [ ] Uses `#[derive(Debug, Error)]` from thiserror
- [ ] Has `#[non_exhaustive]` attribute
- [ ] Each variant has a descriptive `#[error("...")]` message
- [ ] Implements `From<YourError> for dashflow::core::Error`
- [ ] Has a `Result<T>` type alias
- [ ] Has tests for Display output
- [ ] Maps to appropriate `ErrorCategory` via core conversion

### When NOT to Create a New Error Type

- **Simple wrappers**: If your crate just wraps another library, re-export or convert its errors
- **No domain logic**: If errors are just pass-through, use `anyhow::Error`
- **One-off scripts**: Test utilities and scripts don't need typed errors

### Consolidation Patterns

If you find duplicate error types across crates:

1. **Use shared error traits**: Implement `is_retryable()`, `is_network_error()` on your type
2. **Convert to core at boundaries**: Internal errors can be detailed; public APIs should use core types
3. **Document conversion semantics**: Explain which variants map to which categories

## See Also

- [QUICKSTART.md](../QUICKSTART.md) - Getting started with DashFlow
- [API Reference](https://docs.rs/dashflow) - Full API documentation
- [CLI_REFERENCE.md](./CLI_REFERENCE.md) - Command line interface
