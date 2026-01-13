//! # Agent Orchestration Module
//!
//! High-level orchestration of AI agents executing terminal commands.
//!
//! ## TLA+ Specifications
//!
//! This module implements state machines from two TLA+ specifications:
//! - `tla/AgentOrchestration.tla` - Agent lifecycle and command execution
//! - `tla/AgentApproval.tla` - Approval workflow for dangerous operations
//!
//! ## Safety Invariants
//!
//! ### Orchestration (from AgentOrchestration.tla)
//!
//! - **INV-ORCH-1**: No command assigned to multiple agents simultaneously
//! - **INV-ORCH-2**: Every execution has an assigned agent
//! - **INV-ORCH-3**: Terminal used by at most one execution at a time
//! - **INV-ORCH-4**: Command only executes after dependencies complete
//! - **INV-ORCH-5**: Agent has required capabilities for assigned command
//! - **INV-ORCH-6**: Executing agent has valid terminal
//! - **INV-ORCH-7**: Only approved commands can execute
//!
//! ### Approval (from AgentApproval.tla)
//!
//! - **INV-APPROVAL-1**: No request is both approved AND rejected
//! - **INV-APPROVAL-2**: All completed requests have audit entries
//! - **INV-APPROVAL-3**: Pending requests have no completion time
//! - **INV-APPROVAL-4**: Completed requests have valid completion time
//! - **INV-APPROVAL-5**: Request IDs are unique and sequential
//! - **INV-APPROVAL-6**: Timeout only possible if request exceeded timeout
//!
//! ## Architecture
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────────┐
//! │                      Orchestrator                               │
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐     │
//! │  │ AgentPool   │  │CommandQueue │  │   ExecutionManager  │     │
//! │  │             │  │             │  │                     │     │
//! │  │  Agent 1    │  │  Cmd 1 ───→ │  │   Execution 1       │     │
//! │  │  Agent 2    │  │  Cmd 2      │  │   Execution 2       │     │
//! │  │  ...        │  │  ...        │  │   ...               │     │
//! │  └─────────────┘  └─────────────┘  └─────────────────────┘     │
//! │         │                │                    │                 │
//! │         └────────────────┼────────────────────┘                 │
//! │                          ▼                                      │
//! │                  ┌─────────────┐                                │
//! │                  │TerminalPool │                                │
//! │                  │  T1  T2  T3 │                                │
//! │                  └─────────────┘                                │
//! └────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use dterm_core::agent::{Orchestrator, OrchestratorConfig, Capability, Command};
//!
//! // Create orchestrator with configuration
//! let config = OrchestratorConfig {
//!     max_agents: 10,
//!     max_terminals: 5,
//!     max_queue_size: 100,
//! };
//! let mut orchestrator = Orchestrator::new(config);
//!
//! // Spawn an agent with capabilities
//! let agent_id = orchestrator.spawn_agent(&[Capability::Shell, Capability::File])?;
//!
//! // Queue a command
//! let cmd_id = orchestrator.queue_command(Command {
//!     command_type: CommandType::Shell,
//!     required_capabilities: vec![Capability::Shell].into_iter().collect(),
//!     dependencies: Default::default(),
//!     approved: true,
//!     ..Default::default()
//! })?;
//!
//! // Step the orchestrator (assigns commands, starts executions)
//! orchestrator.step();
//! ```

mod approval;
mod command;
mod core;
mod execution;
mod io_driver;
mod orchestrator;
mod runtime;
mod terminal_pool;

pub use approval::{
    Action, ApprovalCallback, ApprovalConfig, ApprovalError, ApprovalManager, ApprovalRequest,
    ApprovalRequestId, ApprovalResult, ApprovalState, AuditEntry, NullApprovalCallback,
};
pub use command::{
    Command, CommandBuilder, CommandId, CommandQueue, CommandQueueError, CommandQueueResult,
    CommandType,
};
pub use core::{Agent, AgentId, AgentState, Capability};
pub use execution::{Execution, ExecutionId, ExecutionManager, ExecutionState};
pub use io_driver::{
    drive_to_completion, DirectIoDriver, ExecutionIoDriver, ExecutionIoDriverExt, IoDriverError,
    IoDriverResult, PollResult, SlotExecutionDriver,
};
pub use orchestrator::{
    Orchestrator, OrchestratorConfig, OrchestratorError, OrchestratorResult, OrchestratorSnapshot,
};
pub use runtime::{
    AgentRuntime, CompletionCallback, CompletionRecord, NullCompletionCallback, RuntimeConfig,
    RuntimeStats, TickResult,
};
pub use terminal_pool::{TerminalPool, TerminalSlot, TerminalSlotId, TerminalSlotState};

#[cfg(test)]
mod tests;
