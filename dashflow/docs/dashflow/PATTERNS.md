# DashFlow Rust: Pattern Library

This document catalogs common workflow patterns for building DashFlow applications. Each pattern includes a description, use cases, code examples, and references to complete implementations.

## Table of Contents

1. [Sequential Pipeline](#sequential-pipeline)
2. [Iterative Refinement](#iterative-refinement)
3. [Conditional Branching](#conditional-branching)
4. [Map-Reduce](#map-reduce)
5. [Multi-Agent Collaboration](#multi-agent-collaboration)
6. [Supervisor-Worker](#supervisor-worker)
7. [Tool Coordination](#tool-coordination)
8. [Human-in-the-Loop](#human-in-the-loop)
9. [State Persistence](#state-persistence)
10. [Streaming Updates](#streaming-updates)

---

## Sequential Pipeline

**Description:** Linear flow of processing steps, each node transforming state and passing to the next.

**Use Cases:**
- ETL pipelines
- Document processing (extract → transform → load)
- Simple agent chains (research → write → publish)

**Pattern Structure:**
```
Node A → Node B → Node C → END
```

**Example:**

```rust
use dashflow::{StateGraph, END};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct PipelineState {
    data: String,
    processed: bool,
}

fn build_pipeline() -> StateGraph<PipelineState> {
    let mut graph = StateGraph::new();

    graph.add_node_from_fn("extract", |mut state| {
        Box::pin(async move {
            // Extract data
            state.data = "extracted data".to_string();
            Ok(state)
        })
    });

    graph.add_node_from_fn("transform", |mut state| {
        Box::pin(async move {
            // Transform data
            state.data = state.data.to_uppercase();
            Ok(state)
        })
    });

    graph.add_node_from_fn("load", |mut state| {
        Box::pin(async move {
            // Load data
            state.processed = true;
            Ok(state)
        })
    });

    // Linear edges
    graph.add_edge("extract", "transform");
    graph.add_edge("transform", "load");
    graph.add_edge("load", END);
    graph.set_entry_point("extract");

    graph
}
```

**Reference Example:** `examples/sequential_workflow.rs`

**Best Practices:**
- Each node should have a single, clear responsibility
- Use typed state fields to enforce contracts between nodes
- Add validation nodes between stages for data quality
- Consider timeouts for each node to prevent pipeline stalls

---

## Iterative Refinement

**Description:** Workflow loops back to improve results until quality criteria are met.

**Use Cases:**
- Content generation with review (write → review → rewrite)
- Self-improving agents
- Optimization loops (propose → evaluate → refine)

**Pattern Structure:**
```
┌─────────┐
│  Write  │ ◄─────┐
└────┬────┘       │
     │            │
     ▼            │
┌─────────┐      │
│ Review  │ ─────┘ (if not good enough)
└────┬────┘
     │
     ▼ (if good enough)
   END
```

**Example:**

```rust
use dashflow::{StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Serialize, Deserialize)]
struct RefinementState {
    draft: String,
    iteration: u32,
    quality_score: f32,
    feedback: Vec<String>,
}

fn build_refinement_graph() -> StateGraph<RefinementState> {
    let mut graph = StateGraph::new();

    graph.add_node_from_fn("writer", |mut state| {
        Box::pin(async move {
            state.iteration += 1;
            // Generate or improve draft based on feedback
            state.draft = format!("Draft {} with improvements", state.iteration);
            Ok(state)
        })
    });

    graph.add_node_from_fn("reviewer", |mut state| {
        Box::pin(async move {
            // Evaluate draft quality
            state.quality_score = 0.7 + (state.iteration as f32 * 0.1);
            if state.quality_score < 0.9 {
                state.feedback.push("Needs more detail".to_string());
            }
            Ok(state)
        })
    });

    // Conditional routing based on quality
    let mut routes = HashMap::new();
    routes.insert("continue".to_string(), "writer".to_string());
    routes.insert("done".to_string(), END.to_string());

    graph.add_conditional_edges(
        "reviewer",
        |state: &RefinementState| {
            if state.quality_score >= 0.9 || state.iteration >= 3 {
                "done".to_string()
            } else {
                "continue".to_string()
            }
        },
        routes,
    );

    graph.add_edge("writer", "reviewer");
    graph.set_entry_point("writer");

    graph
}
```

**Reference Example:** `examples/basic_graph.rs`

**Best Practices:**
- Always include a maximum iteration limit to prevent infinite loops
- Track iteration count in state for debugging
- Store feedback history to understand improvement trajectory
- Use graph-level timeouts as a safety net
- Consider exponential backoff if nodes call external APIs

---

## Conditional Branching

**Description:** Dynamic routing based on state conditions, enabling different execution paths.

**Use Cases:**
- Routing to specialized agents based on query type
- Error handling (retry vs. fallback vs. abort)
- Feature flags (A/B testing different workflows)

**Pattern Structure:**
```
       Classifier
      /    |    \
     /     |     \
  Path1  Path2  Path3
     \     |     /
      \    |    /
        Merger
```

**Example:**

```rust
use dashflow::{StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Serialize, Deserialize)]
struct RoutingState {
    query: String,
    query_type: String,
    result: String,
}

fn build_routing_graph() -> StateGraph<RoutingState> {
    let mut graph = StateGraph::new();

    // Classifier node determines routing
    graph.add_node_from_fn("classifier", |mut state| {
        Box::pin(async move {
            // Classify query to determine routing
            state.query_type = if state.query.contains("weather") {
                "weather".to_string()
            } else if state.query.contains("math") {
                "math".to_string()
            } else {
                "general".to_string()
            };
            Ok(state)
        })
    });

    // Specialized handlers
    graph.add_node_from_fn("weather_handler", |mut state| {
        Box::pin(async move {
            state.result = "Weather info: Sunny".to_string();
            Ok(state)
        })
    });

    graph.add_node_from_fn("math_handler", |mut state| {
        Box::pin(async move {
            state.result = "Math result: 42".to_string();
            Ok(state)
        })
    });

    graph.add_node_from_fn("general_handler", |mut state| {
        Box::pin(async move {
            state.result = "General response".to_string();
            Ok(state)
        })
    });

    // Conditional routing from classifier
    let mut routes = HashMap::new();
    routes.insert("weather".to_string(), "weather_handler".to_string());
    routes.insert("math".to_string(), "math_handler".to_string());
    routes.insert("general".to_string(), "general_handler".to_string());

    graph.add_conditional_edges(
        "classifier",
        |state: &RoutingState| state.query_type.clone(),
        routes,
    );

    // All handlers go to END
    graph.add_edge("weather_handler", END);
    graph.add_edge("math_handler", END);
    graph.add_edge("general_handler", END);

    graph.set_entry_point("classifier");

    graph
}
```

**Reference Example:** `examples/conditional_branching.rs`

**Best Practices:**
- Make routing logic explicit in a dedicated classifier node
- Store routing decision in state for debugging
- Ensure all branches eventually reach END
- Validate routes during graph compilation (all mentioned nodes exist)
- Consider default/fallback routes for unexpected conditions

---

## Map-Reduce

**Description:** Parallel processing of data followed by aggregation of results.

**Use Cases:**
- Parallel research (multiple sources simultaneously)
- Batch processing with aggregation
- Ensemble methods (multiple models, combine predictions)

**Pattern Structure:**
```
     Input
    /  |  \
   /   |   \
 Map1 Map2 Map3  (parallel)
   \   |   /
    \  |  /
    Reduce
```

**Example:**

```rust
use dashflow::{StateGraph, END};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct MapReduceState {
    input: String,
    results: Vec<String>,
    final_result: String,
}

fn build_mapreduce_graph() -> StateGraph<MapReduceState> {
    let mut graph = StateGraph::new();

    graph.add_node_from_fn("input", |state| {
        Box::pin(async move {
            // Prepare input
            Ok(state)
        })
    });

    // Mapper nodes (execute in parallel)
    graph.add_node_from_fn("mapper1", |mut state| {
        Box::pin(async move {
            let result = format!("Map1 processed: {}", state.input);
            state.results.push(result);
            Ok(state)
        })
    });

    graph.add_node_from_fn("mapper2", |mut state| {
        Box::pin(async move {
            let result = format!("Map2 processed: {}", state.input);
            state.results.push(result);
            Ok(state)
        })
    });

    graph.add_node_from_fn("mapper3", |mut state| {
        Box::pin(async move {
            let result = format!("Map3 processed: {}", state.input);
            state.results.push(result);
            Ok(state)
        })
    });

    // Reducer aggregates results
    graph.add_node_from_fn("reducer", |mut state| {
        Box::pin(async move {
            state.final_result = state.results.join(", ");
            Ok(state)
        })
    });

    // Parallel edges from input to all mappers
    graph.add_parallel_edges("input", vec![
        "mapper1".to_string(),
        "mapper2".to_string(),
        "mapper3".to_string(),
    ]);

    // All mappers go to reducer (last mapper's edge determines next node)
    graph.add_edge("mapper1", "reducer");
    graph.add_edge("mapper2", "reducer");
    graph.add_edge("mapper3", "reducer");
    graph.add_edge("reducer", END);

    graph.set_entry_point("input");

    graph
}
```

**Reference Example:** `examples/parallel_map_reduce.rs`

**Best Practices:**
- Use `Arc<T>` for large input data to avoid expensive cloning
- Store results in a collection (Vec, HashMap) in state
- Consider using channels for real-time result streaming
- Set timeouts to prevent one slow mapper from blocking others
- Handle partial failures gracefully (some mappers fail, others succeed)

**Caveat:** Current parallel implementation executes nodes concurrently but only keeps the last node's state. For true map-reduce with state merging, use shared state (e.g., `Arc<Mutex<Vec>>`) or implement a custom merge strategy.

---

## Multi-Agent Collaboration

**Description:** Multiple specialized agents work together, coordinated by a supervisor or through peer collaboration.

**Use Cases:**
- Research systems (researcher + analyst + writer)
- Software development (architect + coder + reviewer)
- Customer service (classifier + specialist agents)

**Pattern Structure:**
```
  Supervisor
  /    |    \
 /     |     \
Agent1 Agent2 Agent3
 \     |     /
  \    |    /
  Synthesizer
```

**Example:**

```rust
use dashflow::{StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Serialize, Deserialize)]
struct MultiAgentState {
    task: String,
    agent_results: HashMap<String, String>,
    synthesis: String,
}

fn build_multiagent_graph() -> StateGraph<MultiAgentState> {
    let mut graph = StateGraph::new();

    // Supervisor delegates tasks
    graph.add_node_from_fn("supervisor", |state| {
        Box::pin(async move {
            println!("Supervisor: Delegating task '{}'", state.task);
            Ok(state)
        })
    });

    // Specialist agents (execute in parallel)
    graph.add_node_from_fn("researcher", |mut state| {
        Box::pin(async move {
            state.agent_results.insert(
                "researcher".to_string(),
                format!("Research findings on '{}'", state.task),
            );
            Ok(state)
        })
    });

    graph.add_node_from_fn("analyst", |mut state| {
        Box::pin(async move {
            state.agent_results.insert(
                "analyst".to_string(),
                format!("Analysis of '{}'", state.task),
            );
            Ok(state)
        })
    });

    graph.add_node_from_fn("writer", |mut state| {
        Box::pin(async move {
            state.agent_results.insert(
                "writer".to_string(),
                format!("Written report on '{}'", state.task),
            );
            Ok(state)
        })
    });

    // Synthesizer combines results
    graph.add_node_from_fn("synthesizer", |mut state| {
        Box::pin(async move {
            state.synthesis = format!(
                "Combined results: {} agents contributed",
                state.agent_results.len()
            );
            Ok(state)
        })
    });

    // Parallel agent execution
    graph.add_parallel_edges("supervisor", vec![
        "researcher".to_string(),
        "analyst".to_string(),
        "writer".to_string(),
    ]);

    graph.add_edge("researcher", "synthesizer");
    graph.add_edge("analyst", "synthesizer");
    graph.add_edge("writer", "synthesizer");
    graph.add_edge("synthesizer", END);

    graph.set_entry_point("supervisor");

    graph
}
```

**Reference Example:** `examples/multi_agent_research.rs`

**Best Practices:**
- Use `HashMap<AgentId, Result>` in state to track which agent produced which result
- Consider agent priorities or dependencies (some agents need others' results)
- Implement timeout per agent to prevent blocking
- Add supervisor retry logic for failed agents
- Use event callbacks to monitor agent progress

---

## Supervisor-Worker

**Description:** A supervisor agent analyzes the task and dynamically routes to appropriate worker agents. Unlike multi-agent collaboration, workers execute sequentially based on supervisor decisions.

**Use Cases:**
- Dynamic task decomposition
- Adaptive workflows (supervisor adjusts plan based on results)
- Expert selection (route to most appropriate specialist)

**Pattern Structure:**
```
Supervisor ──→ Worker 1 ──→ Supervisor
                              │
                              ▼
                           Worker 2 ──→ Supervisor
                                          │
                                          ▼
                                        END
```

**Example:**

```rust
use dashflow::{StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Serialize, Deserialize)]
struct SupervisorState {
    task: String,
    completed_steps: Vec<String>,
    current_plan: Vec<String>,
    results: HashMap<String, String>,
}

fn build_supervisor_graph() -> StateGraph<SupervisorState> {
    let mut graph = StateGraph::new();

    // Supervisor makes routing decisions
    graph.add_node_from_fn("supervisor", |mut state| {
        Box::pin(async move {
            // Decide next worker based on completed steps
            if state.completed_steps.is_empty() {
                state.current_plan = vec!["data_worker".to_string()];
            } else if state.completed_steps.len() == 1 {
                state.current_plan = vec!["analysis_worker".to_string()];
            } else {
                state.current_plan = vec!["done".to_string()];
            }
            Ok(state)
        })
    });

    // Worker 1: Data collection
    graph.add_node_from_fn("data_worker", |mut state| {
        Box::pin(async move {
            state.results.insert("data".to_string(), "collected data".to_string());
            state.completed_steps.push("data_collection".to_string());
            Ok(state)
        })
    });

    // Worker 2: Analysis
    graph.add_node_from_fn("analysis_worker", |mut state| {
        Box::pin(async move {
            state.results.insert("analysis".to_string(), "analyzed data".to_string());
            state.completed_steps.push("analysis".to_string());
            Ok(state)
        })
    });

    // Conditional routing from supervisor
    let mut routes = HashMap::new();
    routes.insert("data_worker".to_string(), "data_worker".to_string());
    routes.insert("analysis_worker".to_string(), "analysis_worker".to_string());
    routes.insert("done".to_string(), END.to_string());

    graph.add_conditional_edges(
        "supervisor",
        |state: &SupervisorState| {
            state.current_plan.first().cloned().unwrap_or("done".to_string())
        },
        routes,
    );

    // Workers return to supervisor
    graph.add_edge("data_worker", "supervisor");
    graph.add_edge("analysis_worker", "supervisor");

    graph.set_entry_point("supervisor");

    graph
}
```

**Best Practices:**
- Track completed steps in state to inform supervisor decisions
- Limit supervisor iterations to prevent infinite loops
- Log supervisor decisions for debugging
- Consider supervisor LLM reasoning for complex routing
- Store worker outputs in structured format (HashMap by worker ID)

---

## Tool Coordination

**Description:** Orchestrate multiple tools, handling tool selection, execution, and result integration.

**Use Cases:**
- Agent tool use (calculator + search + database)
- API integration workflows
- Function calling patterns

**Pattern Structure:**
```
Tool Selector → Tool Executor → Result Integrator → (loop or END)
```

**Example:**

```rust
use dashflow::{StateGraph, ToolNode, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Serialize, Deserialize)]
struct ToolState {
    query: String,
    selected_tool: Option<String>,
    tool_results: HashMap<String, String>,
    final_answer: String,
}

fn build_tool_graph() -> StateGraph<ToolState> {
    let mut graph = StateGraph::new();

    // Tool selector decides which tool to use
    graph.add_node_from_fn("selector", |mut state| {
        Box::pin(async move {
            // Determine which tool to use based on query
            state.selected_tool = Some("calculator".to_string());
            Ok(state)
        })
    });

    // Tool execution nodes
    graph.add_node_from_fn("calculator_tool", |mut state| {
        Box::pin(async move {
            let result = "42".to_string(); // Calculate result
            state.tool_results.insert("calculator".to_string(), result);
            Ok(state)
        })
    });

    graph.add_node_from_fn("search_tool", |mut state| {
        Box::pin(async move {
            let result = "Search results...".to_string();
            state.tool_results.insert("search".to_string(), result);
            Ok(state)
        })
    });

    // Integrator combines tool results
    graph.add_node_from_fn("integrator", |mut state| {
        Box::pin(async move {
            state.final_answer = format!(
                "Answer: {}",
                state.tool_results.get("calculator").unwrap_or(&"none".to_string())
            );
            Ok(state)
        })
    });

    // Conditional routing to tools
    let mut routes = HashMap::new();
    routes.insert("calculator".to_string(), "calculator_tool".to_string());
    routes.insert("search".to_string(), "search_tool".to_string());
    routes.insert("done".to_string(), "integrator".to_string());

    graph.add_conditional_edges(
        "selector",
        |state: &ToolState| {
            state.selected_tool.clone().unwrap_or("done".to_string())
        },
        routes,
    );

    graph.add_edge("calculator_tool", "integrator");
    graph.add_edge("search_tool", "integrator");
    graph.add_edge("integrator", END);

    graph.set_entry_point("selector");

    graph
}
```

**Reference Example:** `examples/tool_using_workflow.rs`

**Best Practices:**
- Use `ToolNode` wrapper for DashFlow tools
- Store tool results in HashMap for easy access
- Implement retry logic for flaky tools
- Add timeout per tool to prevent hangs
- Log tool selections and results for debugging
- Consider parallel tool execution if tools are independent

---

## Human-in-the-Loop

**Description:** Pause execution for human review or approval before continuing.

**Use Cases:**
- Content approval workflows
- High-stakes decision making
- Data labeling and correction

**Pattern Structure:**
```
Automated Node → Checkpoint → (human reviews) → Resume → Continue
```

**Example (Conceptual - requires checkpoint API):**

```rust
use dashflow::{StateGraph, FileCheckpointer, END};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Serialize, Deserialize)]
struct ApprovalState {
    content: String,
    approved: bool,
    reviewer_notes: String,
}

async fn build_approval_workflow() -> Result<(), Box<dyn std::error::Error>> {
    let mut graph: StateGraph<ApprovalState> = StateGraph::new();

    // Generate content
    graph.add_node_from_fn("generator", |mut state| {
        Box::pin(async move {
            state.content = "Generated content...".to_string();
            Ok(state)
        })
    });

    // Wait for approval (checkpoint here)
    graph.add_node_from_fn("approval_checkpoint", |state| {
        Box::pin(async move {
            println!("Content ready for review. Checkpoint saved.");
            // State is checkpointed, human reviews externally
            Ok(state)
        })
    });

    // Process approved content
    graph.add_node_from_fn("publisher", |mut state| {
        Box::pin(async move {
            if state.approved {
                println!("Publishing: {}", state.content);
            }
            Ok(state)
        })
    });

    graph.add_edge("generator", "approval_checkpoint");
    graph.add_edge("approval_checkpoint", "publisher");
    graph.add_edge("publisher", END);
    graph.set_entry_point("generator");

    // Compile with checkpointer
    let checkpointer = FileCheckpointer::new("./checkpoints")?;
    let app = graph.compile()?
        .with_checkpointer(Arc::new(checkpointer))
        .with_thread_id("approval_thread_123".to_string());

    // First run: execute to checkpoint
    let initial_state = ApprovalState {
        content: String::new(),
        approved: false,
        reviewer_notes: String::new(),
    };
    let result = app.invoke(initial_state).await?;

    // (Human reviews externally, modifies state, saves back)

    // Second run: resume from checkpoint with approved state
    // let approved_state = ...load modified state...;
    // let final_result = app.invoke(approved_state).await?;

    Ok(())
}
```

**Reference:** See `crates/dashflow/examples/distributed_checkpointing.rs` for checkpoint mechanics

**Best Practices:**
- Save checkpoint before human interaction point
- Include thread ID for conversation/workflow tracking
- Store human feedback in state for audit trail
- Set expiration time for pending approvals
- Provide UI for humans to review and modify state
- Consider async notification (email, webhook) when checkpoint ready

**Note:** Full interrupt/resume API is planned but not yet implemented (see LANGGRAPH_PLAN.md Phase LG-2).

---

## State Persistence

**Description:** Save and restore graph execution state for resumption, auditing, or recovery.

**Use Cases:**
- Long-running workflows (hours/days)
- Fault tolerance (resume after crashes)
- Audit trails (replay execution)
- Multi-session conversations

**Pattern Structure:**
```
Execute → Checkpoint → (process stops) → Load Checkpoint → Resume
```

**Example:**

```rust
use dashflow::{StateGraph, FileCheckpointer, ThreadId, END};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Serialize, Deserialize)]
struct PersistentState {
    step: u32,
    data: String,
}

async fn persistence_example() -> Result<(), Box<dyn std::error::Error>> {
    let mut graph: StateGraph<PersistentState> = StateGraph::new();

    graph.add_node_from_fn("step1", |mut state| {
        Box::pin(async move {
            state.step = 1;
            state.data = "Step 1 complete".to_string();
            Ok(state)
        })
    });

    graph.add_node_from_fn("step2", |mut state| {
        Box::pin(async move {
            state.step = 2;
            state.data = "Step 2 complete".to_string();
            Ok(state)
        })
    });

    graph.add_edge("step1", "step2");
    graph.add_edge("step2", END);
    graph.set_entry_point("step1");

    // Compile with file-based checkpointing
    let checkpointer = FileCheckpointer::new("./my_checkpoints")?;
    let thread_id = ThreadId::from("workflow_123");

    let app = graph.compile()?
        .with_checkpointer(Arc::new(checkpointer.clone()))
        .with_thread_id(thread_id.clone());

    // Run workflow (checkpoints saved after each node)
    let initial_state = PersistentState {
        step: 0,
        data: String::new(),
    };
    let result = app.invoke(initial_state).await?;

    // Later: Resume from last checkpoint
    let latest = checkpointer.get_latest(thread_id.clone()).await?;
    if let Some(checkpoint) = latest {
        println!("Found checkpoint: {:?}", checkpoint.id);
        // Deserialize and resume
        let resumed_state: PersistentState = serde_json::from_value(checkpoint.state)?;
        println!("Resumed at step {}", resumed_state.step);
    }

    Ok(())
}
```

**Reference Example:** `crates/dashflow/examples/distributed_checkpointing.rs`

**Best Practices:**
- Use `FileCheckpointer` for development, implement database checkpointer for production
- Group related executions with `ThreadId` (e.g., user_id + conversation_id)
- Add metadata to checkpoints (timestamp, user_id, workflow_version)
- Implement checkpoint cleanup (delete old checkpoints)
- Handle schema evolution (old checkpoint + new state struct)
- Consider checkpoint frequency vs. performance trade-off

---

## Streaming Updates

**Description:** Real-time progress updates during graph execution for responsive UIs.

**Use Cases:**
- Live agent responses (ChatGPT-style streaming)
- Progress monitoring for long workflows
- Real-time dashboards

**Pattern Structure:**
```
Execute nodes → Stream events/state → UI updates in real-time
```

**Example:**

```rust
use futures::stream::StreamExt;
use dashflow::{StateGraph, StreamMode, StreamEvent, END};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct StreamingState {
    step: String,
    progress: u32,
}

async fn streaming_example() -> Result<(), Box<dyn std::error::Error>> {
    let mut graph: StateGraph<StreamingState> = StateGraph::new();

    graph.add_node_from_fn("step1", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            state.step = "step1".to_string();
            state.progress = 33;
            Ok(state)
        })
    });

    graph.add_node_from_fn("step2", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            state.step = "step2".to_string();
            state.progress = 66;
            Ok(state)
        })
    });

    graph.add_node_from_fn("step3", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            state.step = "step3".to_string();
            state.progress = 100;
            Ok(state)
        })
    });

    graph.add_edge("step1", "step2");
    graph.add_edge("step2", "step3");
    graph.add_edge("step3", END);
    graph.set_entry_point("step1");

    let app = graph.compile()?.with_stream_mode(StreamMode::Values);

    let initial_state = StreamingState {
        step: String::new(),
        progress: 0,
    };

    // Stream execution
    let mut stream = app.stream(initial_state).await;

    while let Some(event) = stream.next().await {
        match event {
            Ok(StreamEvent::Values(state)) => {
                println!("Progress: {}% ({})", state.progress, state.step);
            }
            Err(e) => {
                eprintln!("Stream error: {}", e);
            }
            _ => {}
        }
    }

    Ok(())
}
```

**Reference Example:** `examples/streaming_workflow.rs`

**Best Practices:**
- Use `StreamMode::Values` for state snapshots (simplest)
- Use `StreamMode::Updates` for efficient deltas (advanced)
- Use `StreamMode::Events` for fine-grained monitoring
- Handle backpressure (bounded channels, drop old events)
- Consider event batching for high-frequency updates
- Add event timestamps for latency monitoring
- Implement reconnection logic for websocket streaming

---

## Pattern Combinations

Real-world applications often combine multiple patterns:

**Example: Complex Research Agent**

```
Supervisor (Conditional Branching)
    ↓
Parallel Researchers (Map-Reduce + Multi-Agent)
    ↓
Iterative Writer/Reviewer (Iterative Refinement)
    ↓
Human Approval (Human-in-the-Loop + Checkpointing)
    ↓
Publisher (Sequential Pipeline)
```

Each stage uses appropriate patterns:
- Conditional branching for query routing
- Map-reduce for parallel research
- Multi-agent collaboration for specialization
- Iterative refinement for quality
- Human-in-the-loop for approval
- Checkpointing for persistence
- Streaming for real-time UI

---

## Choosing the Right Pattern

| Need | Pattern |
|------|---------|
| Simple pipeline | Sequential Pipeline |
| Quality improvement | Iterative Refinement |
| Dynamic routing | Conditional Branching |
| Parallel processing | Map-Reduce |
| Specialist agents | Multi-Agent Collaboration |
| Adaptive planning | Supervisor-Worker |
| External APIs | Tool Coordination |
| Approval workflows | Human-in-the-Loop |
| Long-running tasks | State Persistence |
| Real-time UI | Streaming Updates |

---

## Anti-Patterns to Avoid

### 1. Infinite Loops Without Guards

**Bad:**
```rust
graph.add_conditional_edges("node", |_state| "node".to_string(), routes);
// Always routes back to itself - infinite loop!
```

**Good:**
```rust
graph.add_conditional_edges(
    "node",
    |state: &State| {
        if state.iteration < MAX_ITERATIONS {
            "node".to_string()
        } else {
            END.to_string()
        }
    },
    routes,
);
```

### 2. Large State Cloning

**Bad:**
```rust
#[derive(Clone)]
struct State {
    large_vectors: Vec<Vec<f32>>, // Cloned on every node!
}
```

**Good:**
```rust
#[derive(Clone)]
struct State {
    large_vectors: Arc<Vec<Vec<f32>>>, // Cheap Arc clone
}
```

### 3. Blocking Operations in Nodes

**Bad:**
```rust
graph.add_node_from_fn("node", |state| {
    Box::pin(async move {
        std::thread::sleep(Duration::from_secs(5)); // Blocks tokio thread!
        Ok(state)
    })
});
```

**Good:**
```rust
graph.add_node_from_fn("node", |state| {
    Box::pin(async move {
        tokio::time::sleep(Duration::from_secs(5)).await; // Async sleep
        Ok(state)
    })
});
```

### 4. Unhandled Errors

**Bad:**
```rust
graph.add_node_from_fn("node", |state| {
    Box::pin(async move {
        let result = risky_operation().unwrap(); // Panics on error!
        Ok(state)
    })
});
```

**Good:**
```rust
graph.add_node_from_fn("node", |state| {
    Box::pin(async move {
        match risky_operation().await {
            Ok(result) => {
                // Handle success
                Ok(state)
            }
            Err(e) => {
                // Return error (propagates to caller)
                Err(e.into())
            }
        }
    })
});
```

---

## Further Reading

- **Architecture:** `ARCHITECTURE.md` - System design and implementation details
- **Migration:** `PYTHON_MIGRATION.md` - Guide for Python DashFlow users
- **Best Practices:** `BEST_PRACTICES.md` - Code quality and performance tips
- **Examples:** `crates/dashflow/examples/` - Complete working examples
- **API Docs:** Run `cargo doc --open` for full API reference

---

## Contributing Patterns

Have a pattern that's not listed? Contributions welcome!

1. Implement a working example in `examples/`
2. Add pattern to this document with code snippet
3. Submit PR with description and use cases
