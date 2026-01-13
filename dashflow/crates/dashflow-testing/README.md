# dashflow-testing

Testing utilities for DashFlow applications.

## Overview

This crate provides testing utilities for building robust tests for DashFlow-based applications:

- **MockTool**: A generic mock tool for testing tool-using agents
- **MockEmbeddings**: Re-exported from dashflow core for convenience

## Quick Start

```rust
use dashflow_testing::prelude::*;
use dashflow::core::tools::Tool;

// Create a mock tool with a fixed response
let tool = MockTool::new("calculator")
    .with_description("Performs calculations")
    .with_response("42");

// Or with a dynamic handler
let tool = MockTool::new("calculator")
    .with_handler(|input| Ok(format!("Result: {}", input)));

// Use mock embeddings for testing vector stores
let embeddings = MockEmbeddings::new(384);
```

## MockTool

A configurable mock tool for testing:

```rust
use dashflow_testing::MockTool;
use dashflow::core::tools::{Tool, ToolInput};

// Fixed response
let tool = MockTool::new("search")
    .with_description("Search the web")
    .with_response("Search results: Rust is great");

// Dynamic handler
let tool = MockTool::new("calculator")
    .with_handler(|input| {
        Ok(format!("Calculated: {}", input))
    });

// Test failure scenarios
tool.fail_next();
let result = tool.call(ToolInput::String("test".into())).await;
assert!(result.is_err());

// Verify usage
let tool = MockTool::new("search").with_response("results");
tool.call(ToolInput::String("search for rust".into())).await.unwrap();

assert!(tool.was_called_with("rust"));
assert_eq!(tool.call_count(), 1);
```

## MockEmbeddings

Re-exported from dashflow core for convenience:

```rust
use dashflow_testing::MockEmbeddings;
use dashflow::core::embeddings::Embeddings;

let embeddings = MockEmbeddings::new(384);
// Generates deterministic embeddings based on input text
let vectors = embeddings.embed_documents(&["hello".into()]).await.unwrap();
assert_eq!(vectors[0].len(), 384);
```

## Builder Pattern

Use the builder for more complex tool configurations:

```rust
use dashflow_testing::MockToolBuilder;

let tool = MockToolBuilder::new("complex_tool")
    .description("A complex tool for testing")
    .schema(serde_json::json!({
        "type": "object",
        "properties": {
            "query": { "type": "string" },
            "limit": { "type": "integer" }
        },
        "required": ["query"]
    }))
    .handler(|input| {
        // Parse and process input
        Ok(format!("Processed: {}", input))
    })
    .build();
```

## License

MIT
