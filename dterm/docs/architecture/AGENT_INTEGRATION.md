# Agent-Terminal Integration (Phase 12)

**Status:** COMPLETE (All 5 Steps Done)
**Scope:** dterm-core agent module + domain/pane integration

---

## Goals

- Attach agent executions to real terminal sessions (Pane + Terminal).
- Stream pane output into the terminal parser and capture output for agents.
- Detect completion via pane lifecycle (is_alive/exit_status).
- Preserve existing TLA+ invariants and orchestrator safety checks.
- Provide higher-level runtime for easy integration.

## Non-Goals

- UI integration or rendering changes.
- New domain types or remote connection features.
- Replacing the existing approval workflow.

## Current State (COMPLETE)

- Agent orchestration manages commands, approvals, and execution records.
- Terminal slots can hold real resources (pane + terminal + domain_id).
- Domain and Pane are wired to the agent layer via `Orchestrator.begin_execution()`.
- ExecutionIoDriver trait provides async-agnostic I/O driving.
- Completion detection via `check_execution_completion()`, `check_all_completions()`, and `poll_executions()`.
- Exit status detection transitions executions to Succeeded/Failed with proper resource cleanup.
- **AgentRuntime** provides higher-level integration with callbacks and auto-scheduling.

## Proposed Architecture

### TerminalSlot: Real Resources

Extend `TerminalSlot` to hold the resources needed for a real session.

Proposed fields:
- `pane: Option<Arc<dyn Pane>>`
- `terminal: Option<Terminal>`
- `domain_id: Option<DomainId>`

Responsibilities:
- Own the pane for the lifetime of the execution.
- Maintain the terminal state used to parse output.
- Enforce the one-execution-per-terminal invariant.

### Orchestrator: Domain Reference

Add a domain handle to the orchestrator so it can spawn panes when a command
enters execution:

- `domain: Arc<dyn Domain>` or `domain_registry: Arc<DomainRegistry>`
- Select a domain (default or by command metadata).
- Spawn a pane sized to the requested terminal dimensions.

### Execution: Streaming I/O

Execution should bridge the pane and terminal:

- Read available bytes from the pane.
- Feed bytes into the terminal parser.
- Append raw output to `stdout`/`stderr` buffers (as appropriate).

Suggested mechanism (async-agnostic):
- Add a small `ExecutionIoDriver` trait with `poll()` or `tick()` that can be
  called by a runtime loop.
- Keep the core crate runtime-neutral and let higher layers decide scheduling.

### Completion Detection

Termination conditions:
- `pane.is_alive()` becomes false, then read `exit_status()`.
- If exit status is missing, mark as failed with a clear error message.

### Error Handling

- Domain errors should transition the execution to Failed with context.
- If pane spawn fails, release the terminal slot and report `NoTerminalsAvailable`
  or a new `SpawnFailed` error.

## Execution Flow

1. Agent accepts a command (approved).
2. Orchestrator allocates a terminal slot.
3. Orchestrator spawns a pane from the selected domain.
4. TerminalSlot stores pane + terminal.
5. Execution starts I/O polling loop.
6. Output is parsed into Terminal + buffered for agent consumption.
7. When pane exits, execution state becomes Succeeded/Failed.
8. Terminal slot is released back to the pool.

## Testing Plan

- Add a `MockDomain` and `MockPane` for deterministic output.
- Unit tests for:
  - Spawn failures -> correct state transitions.
  - Output streaming -> terminal state changes and buffers filled.
  - Exit detection -> correct success/failure states.
- Integration test:
  - Full orchestrator flow with a mock domain, verifying invariants.

## Rollout Steps

1. ✅ Extend `TerminalSlot` to hold pane + terminal references.
2. ✅ Add domain handle to `Orchestrator` (default domain or registry).
3. ✅ Create an `ExecutionIoDriver` and wire it to execution lifecycle.
4. ✅ Add tests with `MockDomain` and `MockPane`.
5. ✅ Wire higher-level runtime to drive `poll()` - **AgentRuntime** implemented.

## Resolved Decisions

- **Structured output**: Raw stdout/stderr is captured; structured events can be built on top.
- **Terminal size defaults**: 80x24 default, can be overridden via command metadata.
- **Spawn failures**: `SpawnFailed` error type added to `OrchestratorError`.
- **Output streaming**: Pull-based (tick) via `poll_executions()` and `AgentRuntime::tick()`.

## AgentRuntime API (Step 5)

The `AgentRuntime` struct provides a higher-level interface:

```rust
use dterm_core::agent::{AgentRuntime, RuntimeConfig, CompletionCallback};

// Create runtime
let mut runtime = AgentRuntime::new(RuntimeConfig::default());
runtime.set_default_domain(domain);
runtime.set_completion_callback(Box::new(MyCallback));

// Spawn agents and queue commands
runtime.spawn_agent(&[Capability::Shell])?;
runtime.queue_command(Command::shell(CommandId(0), "echo hello"))?;

// Main loop
loop {
    let result = runtime.tick();
    // result contains: assignments, executions_started, completions, approval_timeouts
}
```

Key features:
- `tick()` - async-agnostic polling method
- `CompletionCallback` - notifications on execution complete
- Auto-assignment and auto-execution (configurable)
- Approval timeout processing
- `run_until_complete()` - convenience method for batch processing

## Related Documents

- `docs/architecture/ARCHITECTURE.md`
- `docs/STRATEGY.md`
- `docs/PENDING_WORK.md`
