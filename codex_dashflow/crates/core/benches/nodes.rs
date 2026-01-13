//! Criterion-based benchmarks for individual agent nodes
//!
//! Run with: cargo bench -p codex-dashflow-core --bench nodes
//!
//! These benchmarks measure the performance of each node in isolation,
//! providing statistical analysis of execution times.

use std::sync::Arc;

use criterion::{criterion_group, criterion_main, Criterion};

use codex_dashflow_core::{
    execpolicy::ExecPolicy,
    graph::build_agent_graph,
    nodes::{
        reasoning::reasoning_node, result_analysis::result_analysis_node,
        tool_execution::mock_tool_execution_node, tool_selection::tool_selection_node,
        user_input::user_input_node,
    },
    runner::{run_agent, RunnerConfig},
    state::{AgentState, Message, ToolCall, ToolResult},
    PromptRegistry,
};

/// Create a minimal state with a user message for benchmarking
fn create_benchmark_state() -> AgentState {
    let mut state = AgentState::new();
    state
        .messages
        .push(Message::user("List files in the current directory"));
    state.use_mock_llm = true;
    state
}

/// Create a state with tool calls for tool selection benchmarking
fn create_state_with_tool_calls() -> AgentState {
    let mut state = create_benchmark_state();
    state.pending_tool_calls.push(ToolCall {
        id: "test-call-1".to_string(),
        tool: "shell".to_string(),
        args: serde_json::json!({"command": "ls -la"}),
    });
    state.pending_tool_calls.push(ToolCall {
        id: "test-call-2".to_string(),
        tool: "read_file".to_string(),
        args: serde_json::json!({"path": "README.md"}),
    });
    state
}

/// Create a state with tool results for result analysis benchmarking
fn create_state_with_results() -> AgentState {
    let mut state = create_benchmark_state();
    state.tool_results.push(ToolResult {
        tool_call_id: "test-call-1".to_string(),
        tool: "shell".to_string(),
        output: "file1.txt\nfile2.txt\ndir1/".to_string(),
        success: true,
        duration_ms: 50,
    });
    state.tool_results.push(ToolResult {
        tool_call_id: "test-call-2".to_string(),
        tool: "read_file".to_string(),
        output: "# README\nThis is the readme content.".to_string(),
        success: true,
        duration_ms: 25,
    });
    state
}

// ============================================================================
// State Creation Benchmarks
// ============================================================================

fn bench_state_creation(c: &mut Criterion) {
    c.bench_function("state_creation", |b| b.iter(AgentState::new));
}

fn bench_state_with_message(c: &mut Criterion) {
    c.bench_function("state_with_message", |b| {
        b.iter(|| {
            let mut state = AgentState::new();
            state.messages.push(Message::user("Hello, world!"));
            state
        })
    });
}

fn bench_state_with_tool_calls(c: &mut Criterion) {
    c.bench_function("state_with_tool_calls", |b| {
        b.iter(create_state_with_tool_calls)
    });
}

// ============================================================================
// Graph Building Benchmarks
// ============================================================================

fn bench_graph_build(c: &mut Criterion) {
    c.bench_function("graph_build", |b| b.iter(build_agent_graph));
}

// ============================================================================
// Individual Node Benchmarks
// ============================================================================

fn bench_user_input_node(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("node_user_input", |b| {
        b.iter(|| {
            let state = create_benchmark_state();
            rt.block_on(user_input_node(state))
        })
    });
}

fn bench_reasoning_node_mock(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("node_reasoning_mock", |b| {
        b.iter(|| {
            let state = create_benchmark_state();
            rt.block_on(reasoning_node(state))
        })
    });
}

fn bench_tool_selection_node(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("node_tool_selection", |b| {
        b.iter(|| {
            let state = create_state_with_tool_calls();
            rt.block_on(tool_selection_node(state))
        })
    });
}

fn bench_tool_selection_with_policy(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let policy = Arc::new(ExecPolicy::default());

    c.bench_function("node_tool_selection_with_policy", |b| {
        b.iter(|| {
            let state = create_state_with_tool_calls();
            rt.block_on(
                codex_dashflow_core::nodes::tool_selection::tool_selection_with_policy(
                    state,
                    policy.clone(),
                ),
            )
        })
    });
}

fn bench_mock_tool_execution_node(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("node_tool_execution_mock", |b| {
        b.iter(|| {
            let state = create_state_with_tool_calls();
            rt.block_on(mock_tool_execution_node(state))
        })
    });
}

fn bench_result_analysis_node(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("node_result_analysis", |b| {
        b.iter(|| {
            let state = create_state_with_results();
            rt.block_on(result_analysis_node(state))
        })
    });
}

// ============================================================================
// Full Agent Loop Benchmarks
// ============================================================================

fn bench_agent_loop_single_turn(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = RunnerConfig::default().with_max_turns(1);

    c.bench_function("agent_loop_single_turn", |b| {
        b.iter(|| {
            let state = create_benchmark_state();
            rt.block_on(run_agent(state, &config))
        })
    });
}

fn bench_agent_loop_three_turns(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = RunnerConfig::default().with_max_turns(3);

    c.bench_function("agent_loop_three_turns", |b| {
        b.iter(|| {
            let state = create_benchmark_state();
            rt.block_on(run_agent(state, &config))
        })
    });
}

// ============================================================================
// Optimize Module Benchmarks
// ============================================================================

fn bench_prompt_registry_defaults(c: &mut Criterion) {
    c.bench_function("prompt_registry_defaults", |b| {
        b.iter(PromptRegistry::with_defaults)
    });
}

fn bench_prompt_registry_get_prompt(c: &mut Criterion) {
    let registry = PromptRegistry::with_defaults();

    c.bench_function("prompt_registry_get_prompt", |b| {
        b.iter(|| registry.get_system_prompt())
    });
}

fn bench_prompt_registry_from_toml(c: &mut Criterion) {
    let toml = r#"
version = 1

[prompts.system]
instruction = "You are a code review assistant."

[[prompts.system.few_shot_examples]]
user_input = "Review this code"
expected_output = "Let me analyze the code..."
score = 0.95
"#;

    c.bench_function("prompt_registry_from_toml", |b| {
        b.iter(|| PromptRegistry::from_toml(toml))
    });
}

// ============================================================================
// Checkpointing Benchmarks
// ============================================================================

fn bench_agent_loop_with_memory_checkpointing(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = RunnerConfig::with_memory_checkpointing().with_max_turns(1);

    c.bench_function("agent_loop_memory_checkpointing", |b| {
        b.iter(|| {
            let state = create_benchmark_state();
            rt.block_on(run_agent(state, &config))
        })
    });
}

fn bench_agent_loop_with_file_checkpointing(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let temp_dir = std::env::temp_dir().join("codex_bench_checkpoints");
    let _ = std::fs::create_dir_all(&temp_dir);
    let config = RunnerConfig::with_file_checkpointing(&temp_dir).with_max_turns(1);

    c.bench_function("agent_loop_file_checkpointing", |b| {
        b.iter(|| {
            let state = create_benchmark_state();
            rt.block_on(run_agent(state, &config))
        })
    });

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

fn bench_checkpointing_overhead(c: &mut Criterion) {
    // This benchmark compares performance with and without checkpointing
    // to measure the overhead of checkpointing
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("checkpointing_overhead");

    // Without checkpointing
    let config_no_checkpoint = RunnerConfig::default().with_max_turns(1);
    group.bench_function("no_checkpointing", |b| {
        b.iter(|| {
            let state = create_benchmark_state();
            rt.block_on(run_agent(state, &config_no_checkpoint))
        })
    });

    // With memory checkpointing
    let config_memory = RunnerConfig::with_memory_checkpointing().with_max_turns(1);
    group.bench_function("memory_checkpointing", |b| {
        b.iter(|| {
            let state = create_benchmark_state();
            rt.block_on(run_agent(state, &config_memory))
        })
    });

    // With file checkpointing
    let temp_dir = std::env::temp_dir().join("codex_bench_overhead_checkpoints");
    let _ = std::fs::create_dir_all(&temp_dir);
    let config_file = RunnerConfig::with_file_checkpointing(&temp_dir).with_max_turns(1);
    group.bench_function("file_checkpointing", |b| {
        b.iter(|| {
            let state = create_benchmark_state();
            rt.block_on(run_agent(state, &config_file))
        })
    });

    group.finish();

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
    state_benches,
    bench_state_creation,
    bench_state_with_message,
    bench_state_with_tool_calls,
);

criterion_group!(graph_benches, bench_graph_build,);

criterion_group!(
    node_benches,
    bench_user_input_node,
    bench_reasoning_node_mock,
    bench_tool_selection_node,
    bench_tool_selection_with_policy,
    bench_mock_tool_execution_node,
    bench_result_analysis_node,
);

criterion_group!(
    agent_loop_benches,
    bench_agent_loop_single_turn,
    bench_agent_loop_three_turns,
);

criterion_group!(
    optimize_benches,
    bench_prompt_registry_defaults,
    bench_prompt_registry_get_prompt,
    bench_prompt_registry_from_toml,
);

criterion_group!(
    checkpointing_benches,
    bench_agent_loop_with_memory_checkpointing,
    bench_agent_loop_with_file_checkpointing,
    bench_checkpointing_overhead,
);

criterion_main!(
    state_benches,
    graph_benches,
    node_benches,
    agent_loop_benches,
    optimize_benches,
    checkpointing_benches,
);
