# DashFlow Tutorial

Welcome to the DashFlow tutorial! This guide will take you from basic concepts to advanced patterns, helping you build production-ready stateful workflows.

## Table of Contents

1. [Getting Started](#part-1-getting-started---simple-sequential-workflow)
2. [Conditional Routing](#part-2-conditional-routing)
3. [Parallel Execution](#part-3-parallel-execution)
4. [Checkpointing](#part-4-checkpointing)
5. [Human-in-the-Loop](#part-5-human-in-the-loop)
6. [Real-World Patterns](#part-6-real-world-patterns)

## Prerequisites

- Rust 1.80 or later
- Basic understanding of async/await in Rust
- Familiarity with serde for serialization

Add DashFlow to your `Cargo.toml`:

```toml
[dependencies]
dashflow = "1.11"
tokio = { version = "1.40", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
```

---

## Part 1: Getting Started - Simple Sequential Workflow

Let's start with the basics: a linear workflow where nodes execute one after another.

### Concept: StateGraph Fundamentals

A **StateGraph** is the core abstraction in DashFlow. It consists of:
- **State**: A struct that holds data flowing through the graph
- **Nodes**: Functions that transform the state
- **Edges**: Connections that define execution order

### Example: Text Processing Pipeline

```rust
use dashflow::{StateGraph, END};
use serde::{Deserialize, Serialize};

// 1. Define your state
#[derive(Clone, Debug, Serialize, Deserialize)]
struct WorkflowState {
    input: String,
    processed: String,
    validated: bool,
    output: String,
}

impl WorkflowState {
    fn new(input: String) -> Self {
        Self {
            input,
            processed: String::new(),
            validated: false,
            output: String::new(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 2. Create the graph
    let mut graph: StateGraph<WorkflowState> = StateGraph::new();

    // 3. Add nodes
    graph.add_node_from_fn("input", |mut state| {
        Box::pin(async move {
            println!("📥 Input: Receiving '{}'", state.input);
            state.processed = format!("Processed: {}", state.input);
            Ok(state)
        })
    });

    graph.add_node_from_fn("process", |mut state| {
        Box::pin(async move {
            println!("⚙️  Process: Transforming data...");
            state.processed = state.processed.to_uppercase();
            Ok(state)
        })
    });

    graph.add_node_from_fn("validate", |mut state| {
        Box::pin(async move {
            println!("✔️  Validate: Checking data quality...");
            state.validated = !state.processed.is_empty();
            Ok(state)
        })
    });

    graph.add_node_from_fn("output", |mut state| {
        Box::pin(async move {
            println!("📤 Output: Generating final result...");
            state.output = format!(
                "Result: {} (validated: {})",
                state.processed, state.validated
            );
            Ok(state)
        })
    });

    // 4. Connect nodes with edges
    graph.set_entry_point("input");
    graph.add_edge("input", "process");
    graph.add_edge("process", "validate");
    graph.add_edge("validate", "output");
    graph.add_edge("output", END);

    // 5. Compile and run
    let app = graph.compile()?;
    let result = app.invoke(WorkflowState::new("Hello World".to_string())).await?;

    println!("Final output: {}", result.final_state.output);
    Ok(())
}
```

### Key Concepts

- **`StateGraph::new()`**: Creates a new graph
- **`add_node_from_fn(name, fn)`**: Adds a node with an async function
- **`set_entry_point(node)`**: Defines where execution starts
- **`add_edge(from, to)`**: Creates a directed edge
- **`END`**: Special constant marking the end of execution
- **`compile()`**: Validates and prepares the graph for execution
- **`invoke(state)`**: Executes the graph with initial state

### Run the Example

```bash
cargo run --example sequential_workflow
```

**Expected Flow**: Input → Process → Validate → Output → END

---

## Part 2: Conditional Routing

Real workflows often need to branch based on conditions. DashFlow supports dynamic routing through conditional edges.

### Concept: Decision Points

Conditional edges allow a node to route to different next nodes based on the current state. This is essential for:
- Classification and routing
- Error handling
- Business logic branching

### Example: Even/Odd Number Router

```rust
use dashflow::{StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BranchState {
    input: i32,
    route: String,
    result: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut graph: StateGraph<BranchState> = StateGraph::new();

    // Classifier node determines routing
    graph.add_node_from_fn("classifier", |mut state| {
        Box::pin(async move {
            if state.input % 2 == 0 {
                state.route = "even".to_string();
                println!("🔀 Routing to EVEN handler");
            } else {
                state.route = "odd".to_string();
                println!("🔀 Routing to ODD handler");
            }
            Ok(state)
        })
    });

    // Handler for even numbers
    graph.add_node_from_fn("handle_even", |mut state| {
        Box::pin(async move {
            state.result = format!("{} is even", state.input);
            Ok(state)
        })
    });

    // Handler for odd numbers
    graph.add_node_from_fn("handle_odd", |mut state| {
        Box::pin(async move {
            state.result = format!("{} is odd", state.input);
            Ok(state)
        })
    });

    // Merge results
    graph.add_node_from_fn("merge", |state| {
        Box::pin(async move {
            println!("🔗 Merge: Result = {}", state.result);
            Ok(state)
        })
    });

    // Build graph with conditional routing
    graph.set_entry_point("classifier");

    // Conditional edge: route based on state.route field
    let mut routes = HashMap::new();
    routes.insert("even".to_string(), "handle_even".to_string());
    routes.insert("odd".to_string(), "handle_odd".to_string());

    graph.add_conditional_edges(
        "classifier",
        |state: &BranchState| state.route.clone(),
        routes,
    );

    // Both branches converge
    graph.add_edge("handle_even", "merge");
    graph.add_edge("handle_odd", "merge");
    graph.add_edge("merge", END);

    let app = graph.compile()?;

    // Test with different inputs
    let result = app.invoke(BranchState {
        input: 42,
        route: String::new(),
        result: String::new(),
    }).await?;

    println!("Path taken: {:?}", result.nodes_executed);
    Ok(())
}
```

### Key Concepts

- **`add_conditional_edges(node, fn, routes)`**: Creates dynamic routing
  - First arg: source node name
  - Second arg: function that returns route key from state
  - Third arg: HashMap mapping route keys to target node names
- **State-based routing**: The routing function examines state and returns a string key
- **Convergence**: Multiple branches can merge back to a common node

### Run the Example

```bash
cargo run --example conditional_branching
```

**Expected Flow**:
- Even number: Classifier → handle_even → Merge → END
- Odd number: Classifier → handle_odd → Merge → END

---

## Part 3: Parallel Execution

For performance and efficiency, you can execute multiple nodes concurrently using parallel edges.

### Concept: Fan-Out, Fan-In

The **Map-Reduce** pattern is common in data processing:
1. **Fan-out**: Distribute work to multiple parallel nodes
2. **Process**: Each node works independently
3. **Fan-in**: Collect results in a reducer node

### Example: Text Analysis with Parallel Mappers

```rust
use dashflow::{StateGraph, END};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MapReduceState {
    input: String,
    results: Vec<String>,
    final_result: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut graph: StateGraph<MapReduceState> = StateGraph::new();

    // Input node
    graph.add_node_from_fn("input", |state| {
        Box::pin(async move {
            println!("📥 Input: Processing '{}'", state.input);
            Ok(state)
        })
    });

    // Parallel mappers
    graph.add_node_from_fn("mapper1_word_count", |mut state| {
        Box::pin(async move {
            let word_count = state.input.split_whitespace().count();
            let result = format!("Words: {}", word_count);
            println!("🔄 Mapper 1: {}", result);
            state.results.push(result);
            Ok(state)
        })
    });

    graph.add_node_from_fn("mapper2_char_count", |mut state| {
        Box::pin(async move {
            let char_count = state.input.len();
            let result = format!("Chars: {}", char_count);
            println!("🔄 Mapper 2: {}", result);
            state.results.push(result);
            Ok(state)
        })
    });

    graph.add_node_from_fn("mapper3_uppercase", |mut state| {
        Box::pin(async move {
            let uppercase_count = state.input.chars().filter(|c| c.is_uppercase()).count();
            let result = format!("Uppercase: {}", uppercase_count);
            println!("🔄 Mapper 3: {}", result);
            state.results.push(result);
            Ok(state)
        })
    });

    // Reducer node
    graph.add_node_from_fn("reduce", |mut state| {
        Box::pin(async move {
            println!("🔀 Reducer: Combining results...");
            state.final_result = state.results.join(", ");
            Ok(state)
        })
    });

    // Build graph with parallel edges
    graph.set_entry_point("input");

    // Fan-out: all mappers run concurrently
    graph.add_parallel_edges(
        "input",
        vec![
            "mapper1_word_count".to_string(),
            "mapper2_char_count".to_string(),
            "mapper3_uppercase".to_string(),
        ],
    );

    // Fan-in: last mapper connects to reducer
    graph.add_edge("mapper3_uppercase", "reduce");
    graph.add_edge("reduce", END);

    let app = graph.compile()?;
    let result = app.invoke(MapReduceState {
        input: "Hello World! This is DashFlow.".to_string(),
        results: Vec::new(),
        final_result: String::new(),
    }).await?;

    println!("Final result: {}", result.final_state.final_result);
    Ok(())
}
```

### Key Concepts

- **`add_parallel_edges(node, vec![targets])`**: Creates concurrent execution paths
- **Concurrent execution**: All parallel nodes start at the same time
- **State merging**: Currently, the last node's state is kept; for full state merging, use shared state (e.g., `Arc<Mutex<Vec>>`)
- **Performance**: Parallel execution can significantly reduce latency for I/O-bound operations

### Run the Example

```bash
cargo run --example parallel_map_reduce
```

**Expected Flow**: Input → [Mapper1 | Mapper2 | Mapper3] (parallel) → Reduce → END

---

## Part 4: Checkpointing

Checkpointing enables persistence, allowing workflows to resume from failures or pauses. This is critical for long-running workflows.

### Concept: State Persistence

DashFlow provides two checkpointer types:
1. **MemoryCheckpointer**: In-memory storage (for testing/development)
2. **FileCheckpointer**: File-based storage (for production/persistence)

Checkpoints save state after each node execution, creating an audit trail and enabling resume from any point.

### Example: Document Processing with Checkpoints

```rust
use dashflow::{FileCheckpointer, StateGraph};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DocumentState {
    content: String,
    stage: String,
    word_count: usize,
    summary: Option<String>,
    tags: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut graph: StateGraph<DocumentState> = StateGraph::new();

    // Parse document
    graph.add_node_from_fn("parse", |mut state| {
        Box::pin(async move {
            println!("📄 Parsing document...");
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            state.word_count = state.content.split_whitespace().count();
            state.stage = "parsed".to_string();
            Ok(state)
        })
    });

    // Extract summary
    graph.add_node_from_fn("summarize", |mut state| {
        Box::pin(async move {
            println!("📝 Extracting summary...");
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            let summary = state.content.chars().take(50).collect::<String>() + "...";
            state.summary = Some(summary);
            state.stage = "summarized".to_string();
            Ok(state)
        })
    });

    // Extract tags
    graph.add_node_from_fn("tag", |mut state| {
        Box::pin(async move {
            println!("🏷️  Extracting tags...");
            tokio::time::sleep(std::time::Duration::from_millis(350)).await;
            state.tags = state
                .content
                .split_whitespace()
                .filter(|w| w.len() > 5)
                .take(5)
                .map(|s| s.to_lowercase())
                .collect();
            state.stage = "tagged".to_string();
            Ok(state)
        })
    });

    // Build linear workflow
    graph.set_entry_point("parse");
    graph.add_edge("parse", "summarize");
    graph.add_edge("summarize", "tag");
    graph.add_edge("tag", dashflow::END);

    // Compile with checkpointer
    let checkpoint_dir = PathBuf::from("target/checkpoints");
    let checkpointer = FileCheckpointer::new(checkpoint_dir);

    let app = graph
        .compile()?
        .with_checkpointer(checkpointer)
        .with_thread_id("doc-123".to_string());

    // First execution
    let initial_state = DocumentState {
        content: "Artificial intelligence and machine learning are transforming software development.".to_string(),
        stage: "initial".to_string(),
        word_count: 0,
        summary: None,
        tags: Vec::new(),
    };

    println!("🚀 Starting checkpointed workflow...\n");
    let result = app.invoke(initial_state).await?;

    println!("\n✅ Workflow completed!");
    println!("Stage: {}", result.final_state.stage);
    println!("Word count: {}", result.final_state.word_count);
    println!("Summary: {}", result.final_state.summary.unwrap_or_default());
    println!("Tags: {:?}", result.final_state.tags);

    // Resume from checkpoint (if needed)
    // You can restart with the same thread_id to continue from last checkpoint
    println!("\n💾 Checkpoints saved to target/checkpoints/");

    Ok(())
}
```

### Key Concepts

- **`with_checkpointer(checkpointer)`**: Enables state persistence
- **`with_thread_id(id)`**: Identifies the execution thread for checkpoint isolation
- **Thread isolation**: Different thread IDs maintain separate checkpoint histories
- **Resume capability**: Restart with same thread_id to continue from last checkpoint
- **Audit trail**: Checkpoints create a complete history of state changes

### Checkpoint Types

```rust
// In-memory (development/testing)
use dashflow::MemoryCheckpointer;
let checkpointer = MemoryCheckpointer::new();

// File-based (production)
use dashflow::FileCheckpointer;
use std::path::PathBuf;
let checkpointer = FileCheckpointer::new(PathBuf::from("./checkpoints"));
```

### Run the Example

```bash
cargo run --example checkpointing_workflow
```

**Checkpoint Features**:
- State saved after each node
- Resume from any checkpoint
- Full execution history
- Thread-based isolation

---

## Part 5: Human-in-the-Loop

Many workflows require human intervention for decision-making, quality control, or handling edge cases. DashFlow supports human-in-the-loop patterns through conditional routing and quality checks.

### Concept: Quality Gates and Escalation

Human-in-the-loop workflows typically have:
1. **Automated processing**: AI agents handle routine cases
2. **Quality check**: Determine if human review is needed
3. **Escalation logic**: Route complex cases to humans
4. **Human review**: Human makes final decision or provides input
5. **Resolution**: Workflow completes with enhanced outcome

### Example: Customer Service Router with Escalation

This example demonstrates a multi-agent system with human escalation:

```rust
use dashflow::{StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Serialize, Deserialize)]
struct CustomerServiceState {
    customer_query: String,
    intent: String,
    specialist_response: String,
    requires_human: bool,
    escalation_reason: String,
    resolution: String,
    satisfaction_score: Option<f32>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut graph = StateGraph::new();

    // Intent classifier
    graph.add_node_from_fn("intent_classifier", |mut state: CustomerServiceState| {
        Box::pin(async move {
            let query_lower = state.customer_query.to_lowercase();
            state.intent = if query_lower.contains("refund") {
                "billing".to_string()
            } else if query_lower.contains("crash") {
                "tech_support".to_string()
            } else {
                "sales".to_string()
            };
            println!("🎯 Intent: {}", state.intent);
            Ok(state)
        })
    });

    // Billing specialist
    graph.add_node_from_fn("billing_specialist", |mut state: CustomerServiceState| {
        Box::pin(async move {
            println!("💰 Billing specialist handling query...");
            state.specialist_response = "I can help with that billing issue.".to_string();

            // Refund requests need human approval
            if state.customer_query.to_lowercase().contains("refund") {
                state.requires_human = true;
                state.escalation_reason = "Refund requests require manager approval".to_string();
            }
            Ok(state)
        })
    });

    // Tech support specialist
    graph.add_node_from_fn("tech_specialist", |mut state: CustomerServiceState| {
        Box::pin(async move {
            println!("🔧 Tech specialist handling query...");
            state.specialist_response = "Let me help troubleshoot that issue.".to_string();

            // Critical issues escalate to senior engineer
            if state.customer_query.to_lowercase().contains("crash") {
                state.requires_human = true;
                state.escalation_reason = "Critical issue requires senior engineer".to_string();
            }
            Ok(state)
        })
    });

    // Sales specialist
    graph.add_node_from_fn("sales_specialist", |mut state: CustomerServiceState| {
        Box::pin(async move {
            println!("📈 Sales specialist handling query...");
            state.specialist_response = "I'd be happy to discuss our offerings.".to_string();
            Ok(state)
        })
    });

    // Quality check node
    graph.add_node_from_fn("quality_check", |state: CustomerServiceState| {
        Box::pin(async move {
            println!("🔍 Quality check: Human review needed = {}", state.requires_human);
            Ok(state)
        })
    });

    // Human review (simulated)
    graph.add_node_from_fn("human_review", |mut state: CustomerServiceState| {
        Box::pin(async move {
            println!("👤 Human review: {}", state.escalation_reason);
            state.resolution = format!(
                "[REVIEWED BY HUMAN] {} Additional context added.",
                state.specialist_response
            );
            state.satisfaction_score = Some(0.95);
            Ok(state)
        })
    });

    // Complete
    graph.add_node_from_fn("complete", |mut state: CustomerServiceState| {
        Box::pin(async move {
            if state.resolution.is_empty() {
                state.resolution = state.specialist_response.clone();
                state.satisfaction_score = Some(0.85);
            }
            println!("✅ Resolution: {}", state.resolution);
            println!("   Satisfaction: {:.0}%", state.satisfaction_score.unwrap_or(0.0) * 100.0);
            Ok(state)
        })
    });

    // Build graph with routing
    graph.set_entry_point("intent_classifier");

    // Intent-based routing
    let mut intent_routes = HashMap::new();
    intent_routes.insert("billing".to_string(), "billing_specialist".to_string());
    intent_routes.insert("tech_support".to_string(), "tech_specialist".to_string());
    intent_routes.insert("sales".to_string(), "sales_specialist".to_string());

    graph.add_conditional_edges(
        "intent_classifier",
        |state: &CustomerServiceState| state.intent.clone(),
        intent_routes,
    );

    // All specialists go to quality check
    graph.add_edge("billing_specialist", "quality_check");
    graph.add_edge("tech_specialist", "quality_check");
    graph.add_edge("sales_specialist", "quality_check");

    // Quality check routing
    let mut quality_routes = HashMap::new();
    quality_routes.insert("human".to_string(), "human_review".to_string());
    quality_routes.insert("auto".to_string(), "complete".to_string());

    graph.add_conditional_edges(
        "quality_check",
        |state: &CustomerServiceState| {
            if state.requires_human {
                "human".to_string()
            } else {
                "auto".to_string()
            }
        },
        quality_routes,
    );

    graph.add_edge("human_review", "complete");
    graph.add_edge("complete", END);

    let app = graph.compile()?;

    // Test case: refund request (requires human)
    println!("🚀 Test: Refund request\n");
    let result = app.invoke(CustomerServiceState {
        customer_query: "I need a refund for my last charge".to_string(),
        intent: String::new(),
        specialist_response: String::new(),
        requires_human: false,
        escalation_reason: String::new(),
        resolution: String::new(),
        satisfaction_score: None,
    }).await?;

    println!("\nPath: {:?}", result.nodes_executed);

    Ok(())
}
```

### Key Concepts

- **Quality gates**: Automated checks determine if human intervention is needed
- **Escalation flags**: State fields (`requires_human`, `escalation_reason`) control routing
- **Conditional routing**: Quality check node routes to human_review or auto-complete
- **Enhanced outcomes**: Human review increases satisfaction scores
- **Audit trail**: Track which cases required human intervention

### Run the Example

```bash
cargo run --example customer_service_router
```

**Expected Flow**:
- Simple query: Classifier → Specialist → Quality Check → Complete
- Complex query: Classifier → Specialist → Quality Check → Human Review → Complete

---

## Part 6: Real-World Patterns

Let's explore production-ready patterns that combine multiple concepts.

### Pattern 1: Batch Processing Pipeline

Handle multiple items with error recovery and retry logic.

```rust
// Simplified example - see examples/batch_processing_pipeline.rs for full version
use dashflow::{FileCheckpointer, StateGraph};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct BatchItem {
    id: String,
    data: String,
    status: String, // "pending", "processing", "success", "failed"
    retry_count: u32,
    error_message: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct BatchState {
    items: Vec<BatchItem>,
    max_retries: u32,
    processed_items: u32,
    failed_items: u32,
}
```

**Key Features**:
- Batch input handling (Vec<BatchItem>)
- Per-item status tracking (pending, processing, success, failed)
- Validation stage (pre-processing checks)
- Parallel processing simulation
- Error handling with retry logic (max_retries limit)
- Checkpointing for resume capability
- Results aggregation (success rate, error logs)

```bash
cargo run --example batch_processing_pipeline
```

**Use Cases**: ETL pipelines, data processing jobs, bulk API operations

---

### Pattern 2: Multi-Agent Research System

Coordinate multiple specialized agents for complex tasks.

```rust
// See examples/multi_agent_research.rs for full implementation
```

**Key Features**:
- Task decomposition (break complex query into subtasks)
- Parallel research (multiple agents work concurrently)
- Information synthesis (combine findings)
- Quality validation (ensure completeness)

```bash
cargo run --example multi_agent_research
```

**Use Cases**: Research automation, data aggregation, competitive analysis

---

### Pattern 3: Code Review Workflow

Automated code review with multiple analysis stages.

```rust
// See examples/code_review_workflow.rs for full implementation
```

**Key Features**:
- Multi-stage analysis (style, security, performance, tests)
- Severity classification (blocking, warning, info)
- Aggregated reports
- Actionable feedback

```bash
cargo run --example code_review_workflow
```

**Use Cases**: CI/CD pipelines, code quality gates, automated reviews

---

### Pattern 4: Financial Analysis Agent

Complex data analysis with conditional logic.

```rust
// See examples/financial_analysis_agent.rs for full implementation
```

**Key Features**:
- Data ingestion and validation
- Risk assessment
- Conditional analysis paths (high/medium/low risk)
- Report generation with recommendations

```bash
cargo run --example financial_analysis_agent
```

**Use Cases**: Financial analysis, risk assessment, compliance checking

---

## Best Practices

### 1. State Design

**DO:**
- Keep state flat and simple
- Use `Option<T>` for fields computed later
- Implement `Clone`, `Serialize`, `Deserialize`
- Use descriptive field names

**DON'T:**
- Store large blobs in state (use references/IDs instead)
- Use complex nested structures unnecessarily
- Store non-serializable types (unless using custom serialization)

```rust
// Good
#[derive(Clone, Serialize, Deserialize)]
struct State {
    user_id: String,
    status: String,
    result: Option<String>,
}

// Avoid
#[derive(Clone, Serialize, Deserialize)]
struct State {
    user: ComplexUserObject,
    nested: HashMap<String, Vec<ComplexType>>,
}
```

---

### 2. Node Design

**DO:**
- Keep nodes focused (single responsibility)
- Use async operations for I/O
- Handle errors gracefully
- Log important state changes

**DON'T:**
- Mix multiple concerns in one node
- Block the async runtime (use `tokio::task::spawn_blocking` for CPU-heavy work)
- Ignore errors silently

```rust
// Good - focused node
graph.add_node_from_fn("validate_email", |mut state| {
    Box::pin(async move {
        state.email_valid = email_regex.is_match(&state.email);
        if !state.email_valid {
            println!("❌ Invalid email: {}", state.email);
        }
        Ok(state)
    })
});

// Avoid - too many responsibilities
graph.add_node_from_fn("do_everything", |mut state| {
    Box::pin(async move {
        // validate, process, store, send email, log, analyze...
        Ok(state)
    })
});
```

---

### 3. Error Handling

**DO:**
- Return `Result<State, Box<dyn std::error::Error>>` from nodes
- Use checkpointing for long-running workflows
- Implement retry logic for transient failures
- Log errors with context

**DON'T:**
- Panic in nodes (use proper error handling)
- Silently swallow errors
- Retry indefinitely (use max_retries)

```rust
graph.add_node_from_fn("api_call", |mut state| {
    Box::pin(async move {
        match make_api_call(&state.request).await {
            Ok(response) => {
                state.response = Some(response);
                state.status = "success".to_string();
            }
            Err(e) => {
                eprintln!("❌ API call failed: {}", e);
                state.retry_count += 1;
                state.status = if state.retry_count < state.max_retries {
                    "retry".to_string()
                } else {
                    "failed".to_string()
                };
            }
        }
        Ok(state)
    })
});
```

---

### 4. Checkpointing

**When to use:**
- Workflows taking > 10 seconds
- External API calls (network failures)
- Batch processing (partial failures)
- User-interactive workflows (human-in-the-loop)

**When to skip:**
- Simple, fast workflows (< 1 second)
- In-memory computations only
- No failure recovery needed

```rust
// Use FileCheckpointer for production
let app = graph
    .compile()?
    .with_checkpointer(FileCheckpointer::new(PathBuf::from("./checkpoints")))
    .with_thread_id(format!("job-{}", job_id));

// Use MemoryCheckpointer for testing
let app = graph
    .compile()?
    .with_checkpointer(MemoryCheckpointer::new())
    .with_thread_id("test-thread".to_string());
```

---

### 5. Performance Optimization

**Techniques:**
- Use parallel edges for independent operations
- Minimize state size (reduces serialization overhead)
- Use `--release` builds for production
- Profile with `cargo flamegraph` for bottlenecks

```rust
// Parallel execution for independent tasks
graph.add_parallel_edges(
    "prepare",
    vec![
        "analyze_sentiment".to_string(),
        "extract_entities".to_string(),
        "detect_language".to_string(),
    ],
);
```

---

## Testing Your Workflows

### Unit Testing Nodes

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_validation_node() {
        let state = WorkflowState {
            email: "test@example.com".to_string(),
            email_valid: false,
        };

        let graph = build_graph();
        let result = graph.invoke(state).await.unwrap();

        assert!(result.final_state.email_valid);
    }
}
```

### Integration Testing Workflows

```rust
#[tokio::test]
async fn test_full_workflow() {
    let graph = build_customer_service_graph();
    let app = graph.compile().unwrap();

    let result = app.invoke(CustomerServiceState {
        customer_query: "I need a refund".to_string(),
        // ... other fields
    }).await.unwrap();

    assert!(result.nodes_executed.contains(&"human_review".to_string()));
    assert!(result.final_state.satisfaction_score.unwrap() > 0.9);
}
```

---

## Debugging Tips

### 1. Enable Logging

```rust
// Add logging to nodes
graph.add_node_from_fn("process", |mut state| {
    Box::pin(async move {
        println!("🔍 DEBUG: state.field = {:?}", state.field);
        // ... node logic
        Ok(state)
    })
});
```

### 2. Inspect Execution Path

```rust
let result = app.invoke(initial_state).await?;
println!("Nodes executed: {:?}", result.nodes_executed);
println!("Final state: {:?}", result.final_state);
```

### 3. Checkpoint Inspection

Checkpoints are saved as files. You can inspect them to understand state at each step:

```bash
ls target/checkpoints/
cat target/checkpoints/thread-123/checkpoint-002.json
```

---

## Next Steps

Congratulations! You now understand DashFlow fundamentals and advanced patterns. Here's what to explore next:

1. **Build Your Own Workflow**: Start with a simple sequential workflow and gradually add complexity
2. **Explore Examples**: Check `examples/` directory for 14+ production-ready examples
3. **Read Architecture Docs**: See `ARCHITECTURE.md` for internal design details
4. **Performance Tuning**: See `PERFORMANCE.md` for benchmarks and optimization guide
5. **Troubleshooting**: See `TROUBLESHOOTING.md` for common issues and solutions

### Additional Resources

- **API Documentation**: Run `cargo doc --open` to browse full API docs
- **Examples**: All examples in `crates/dashflow/examples/`
- **Source Code**: Explore `crates/dashflow/src/` for implementation details

---

## Complete Example Index

| Example | Pattern | Concepts |
|---------|---------|----------|
| `sequential_workflow.rs` | Linear pipeline | Basic nodes, edges, state |
| `conditional_branching.rs` | Decision routing | Conditional edges, HashMap routing |
| `parallel_map_reduce.rs` | Concurrent processing | Parallel edges, fan-out/fan-in |
| `checkpointing_workflow.rs` | State persistence | FileCheckpointer, resume capability |
| `customer_service_router.rs` | Multi-agent + HITL | Intent classification, escalation, quality gates |
| `batch_processing_pipeline.rs` | Batch processing | Retry logic, error handling, batch state |
| `multi_agent_research.rs` | Agent coordination | Task decomposition, synthesis |
| `code_review_workflow.rs` | Analysis pipeline | Multi-stage analysis, severity classification |
| `financial_analysis_agent.rs` | Complex analysis | Risk assessment, conditional paths |
| `tool_using_workflow.rs` | Tool integration | External tool calls |
| `streaming_workflow.rs` | Streaming output | Real-time updates |
| `graph_events.rs` | Event system | Event emission, subscriptions |
| `dashflow_integration.rs` | DashFlow integration | Using dashflow traits |
| `basic_graph.rs` | Minimal example | Simplest possible graph |

---

## Glossary

- **StateGraph**: Core abstraction representing a workflow as a directed graph of nodes
- **Node**: A function that transforms state (typically async)
- **Edge**: Connection between nodes defining execution order
- **Conditional Edge**: Dynamic routing based on state
- **Parallel Edge**: Fan-out to multiple concurrent nodes
- **Checkpointer**: Persistence layer for state snapshots
- **Thread ID**: Identifier for isolated execution context
- **Human-in-the-Loop (HITL)**: Pattern where humans review/approve automated decisions
- **Quality Gate**: Checkpoint where automated validation determines next steps
- **Escalation**: Routing complex cases to higher-level handlers (often human)

---

## Getting Help

If you encounter issues:

1. Check `TROUBLESHOOTING.md` for common problems
2. Review example code in `examples/` directory
3. Inspect checkpoint files to understand state changes
4. Enable debug logging to trace execution
5. File an issue on GitHub with minimal reproduction

Happy building with DashFlow! 🚀
