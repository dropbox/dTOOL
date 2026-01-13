//! Agent Runtime - Higher-level orchestration loop.
//!
//! ## Phase 12 Step 5: Runtime Integration
//!
//! This module provides the `AgentRuntime` struct that wraps the `Orchestrator`
//! and provides a higher-level interface for managing agent execution lifecycle.
//!
//! ## Design Principles
//!
//! - **Async-agnostic**: Like `ExecutionIoDriver`, the runtime uses `tick()` methods
//!   that can be called by any scheduling system (sync loop, tokio, etc.)
//! - **Callback-based**: Completion events are delivered via callbacks
//! - **Full lifecycle**: Manages approval → assignment → execution → completion
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                          AgentRuntime                                    │
//! │  ┌─────────────────────────────────────────────────────────────────┐    │
//! │  │                      Orchestrator                                │    │
//! │  │  ┌──────────┐  ┌──────────┐  ┌───────────────┐  ┌────────────┐  │    │
//! │  │  │ AgentPool│  │ CmdQueue │  │ ExecutionMgr  │  │ TermPool   │  │    │
//! │  │  └──────────┘  └──────────┘  └───────────────┘  └────────────┘  │    │
//! │  └─────────────────────────────────────────────────────────────────┘    │
//! │                              │                                           │
//! │                              ▼                                           │
//! │                    ┌───────────────────┐                                 │
//! │                    │ CompletionCallback │─────▶ UI / External Systems   │
//! │                    └───────────────────┘                                 │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use dterm_core::agent::{AgentRuntime, RuntimeConfig, CompletionCallback};
//! use std::sync::Arc;
//!
//! // Create callback
//! struct MyCallback;
//! impl CompletionCallback for MyCallback {
//!     fn on_completion(&self, exec_id: ExecutionId, exit_code: i32, success: bool) {
//!         println!("Execution {} completed: exit={}, success={}", exec_id, exit_code, success);
//!     }
//! }
//!
//! // Create runtime
//! let mut runtime = AgentRuntime::new(RuntimeConfig::default());
//! runtime.set_completion_callback(Box::new(MyCallback));
//! runtime.set_default_domain(domain);
//!
//! // Main loop
//! loop {
//!     let tick_result = runtime.tick();
//!     if tick_result.completions > 0 {
//!         println!("{} executions completed", tick_result.completions);
//!     }
//!     std::thread::sleep(Duration::from_millis(10));
//! }
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::{
    AgentId, AgentState, ApprovalRequestId, Capability, Command, CommandId, Execution, ExecutionId,
    ExecutionState, Orchestrator, OrchestratorConfig, OrchestratorError, OrchestratorResult,
};
use crate::domain::Domain;

/// Callback trait for execution completion events.
///
/// Implementations receive notifications when executions complete,
/// allowing UI layers or external systems to respond.
pub trait CompletionCallback: Send + Sync {
    /// Called when an execution completes.
    ///
    /// # Arguments
    ///
    /// * `exec_id` - The execution that completed
    /// * `exit_code` - The process exit code
    /// * `success` - True if exit code was 0
    /// * `agent_id` - The agent that ran the execution
    /// * `command_id` - The command that was executed
    fn on_completion(
        &self,
        exec_id: ExecutionId,
        exit_code: i32,
        success: bool,
        agent_id: AgentId,
        command_id: CommandId,
    );

    /// Called when an execution fails to start (e.g., spawn failure).
    ///
    /// # Arguments
    ///
    /// * `agent_id` - The agent that attempted execution
    /// * `command_id` - The command that failed
    /// * `error` - Description of the failure
    fn on_execution_failed(&self, agent_id: AgentId, command_id: CommandId, error: &str) {
        // Default: do nothing
        let _ = (agent_id, command_id, error);
    }

    /// Called when an agent is spawned.
    fn on_agent_spawned(&self, agent_id: AgentId) {
        let _ = agent_id;
    }

    /// Called when a command is queued.
    fn on_command_queued(&self, command_id: CommandId) {
        let _ = command_id;
    }

    /// Called when a command is assigned to an agent.
    fn on_command_assigned(&self, command_id: CommandId, agent_id: AgentId) {
        let _ = (command_id, agent_id);
    }

    /// Called when execution begins.
    fn on_execution_started(&self, exec_id: ExecutionId, agent_id: AgentId, command_id: CommandId) {
        let _ = (exec_id, agent_id, command_id);
    }
}

/// No-op completion callback for testing or when callbacks aren't needed.
pub struct NullCompletionCallback;

/// Implement CompletionCallback for `Arc<T>` where T: CompletionCallback.
///
/// This allows sharing callbacks between multiple owners (e.g., for testing).
impl<T: CompletionCallback + ?Sized> CompletionCallback for Arc<T> {
    fn on_completion(
        &self,
        exec_id: ExecutionId,
        exit_code: i32,
        success: bool,
        agent_id: AgentId,
        command_id: CommandId,
    ) {
        (**self).on_completion(exec_id, exit_code, success, agent_id, command_id);
    }

    fn on_execution_failed(&self, agent_id: AgentId, command_id: CommandId, error: &str) {
        (**self).on_execution_failed(agent_id, command_id, error);
    }

    fn on_agent_spawned(&self, agent_id: AgentId) {
        (**self).on_agent_spawned(agent_id);
    }

    fn on_command_queued(&self, command_id: CommandId) {
        (**self).on_command_queued(command_id);
    }

    fn on_command_assigned(&self, command_id: CommandId, agent_id: AgentId) {
        (**self).on_command_assigned(command_id, agent_id);
    }

    fn on_execution_started(&self, exec_id: ExecutionId, agent_id: AgentId, command_id: CommandId) {
        (**self).on_execution_started(exec_id, agent_id, command_id);
    }
}

impl CompletionCallback for NullCompletionCallback {
    fn on_completion(
        &self,
        _exec_id: ExecutionId,
        _exit_code: i32,
        _success: bool,
        _agent_id: AgentId,
        _command_id: CommandId,
    ) {
        // No-op
    }
}

/// Configuration for the agent runtime.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Orchestrator configuration.
    pub orchestrator: OrchestratorConfig,
    /// Whether to auto-assign commands to agents.
    pub auto_assign: bool,
    /// Whether to auto-start executions for assigned agents.
    pub auto_execute: bool,
    /// Interval between approval timeout checks.
    pub approval_timeout_interval: Duration,
    /// Maximum executions to start per tick.
    pub max_executions_per_tick: usize,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            orchestrator: OrchestratorConfig::default(),
            auto_assign: true,
            auto_execute: true,
            approval_timeout_interval: Duration::from_secs(1),
            max_executions_per_tick: 10,
        }
    }
}

/// Result of a single runtime tick.
#[derive(Debug, Clone, Default)]
pub struct TickResult {
    /// Number of commands assigned to agents.
    pub assignments: usize,
    /// Number of executions started.
    pub executions_started: usize,
    /// Number of executions that completed.
    pub completions: usize,
    /// Number of approval requests that timed out.
    pub approval_timeouts: usize,
    /// Errors encountered during the tick.
    pub errors: Vec<String>,
}

impl TickResult {
    /// Returns true if any work was done.
    pub fn had_activity(&self) -> bool {
        self.assignments > 0
            || self.executions_started > 0
            || self.completions > 0
            || self.approval_timeouts > 0
    }

    /// Returns true if there were any errors.
    pub fn had_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

/// Execution completion record for tracking completed executions.
#[derive(Debug, Clone)]
pub struct CompletionRecord {
    /// The execution ID.
    pub exec_id: ExecutionId,
    /// The agent that ran the execution.
    pub agent_id: AgentId,
    /// The command that was executed.
    pub command_id: CommandId,
    /// The exit code.
    pub exit_code: i32,
    /// Whether the execution succeeded (exit code 0).
    pub success: bool,
    /// When the execution completed.
    pub completed_at: Instant,
}

/// Higher-level runtime for agent orchestration.
///
/// The `AgentRuntime` wraps an `Orchestrator` and provides:
/// - A `tick()` method for integration with any scheduling system
/// - Completion callbacks for UI integration
/// - Full execution lifecycle management
/// - Auto-assignment and auto-execution (configurable)
///
/// ## Thread Safety
///
/// The runtime is `Send` but not `Sync` - it should be owned by a single
/// thread/task that calls `tick()` periodically. Callbacks are `Send + Sync`
/// and can be shared.
pub struct AgentRuntime {
    /// The underlying orchestrator.
    orchestrator: Orchestrator,
    /// Runtime configuration.
    config: RuntimeConfig,
    /// Completion callback.
    completion_callback: Option<Box<dyn CompletionCallback>>,
    /// Last approval timeout check.
    last_timeout_check: Instant,
    /// Recent completions (for debugging/monitoring).
    recent_completions: Vec<CompletionRecord>,
    /// Maximum recent completions to keep.
    max_recent_completions: usize,
}

impl AgentRuntime {
    /// Create a new agent runtime with the given configuration.
    pub fn new(config: RuntimeConfig) -> Self {
        Self {
            orchestrator: Orchestrator::new(config.orchestrator.clone()),
            config,
            completion_callback: None,
            last_timeout_check: Instant::now(),
            recent_completions: Vec::new(),
            max_recent_completions: 100,
        }
    }

    /// Create a runtime with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(RuntimeConfig::default())
    }

    /// Set the completion callback.
    pub fn set_completion_callback(&mut self, callback: Box<dyn CompletionCallback>) {
        self.completion_callback = Some(callback);
    }

    /// Set the default domain for pane spawning.
    ///
    /// See [`Orchestrator::set_default_domain`].
    pub fn set_default_domain(&mut self, domain: Arc<dyn Domain>) {
        self.orchestrator.set_default_domain(domain);
    }

    /// Check if domain support is enabled.
    pub fn has_domain_support(&self) -> bool {
        self.orchestrator.has_domain_support()
    }

    /// Get a reference to the underlying orchestrator.
    pub fn orchestrator(&self) -> &Orchestrator {
        &self.orchestrator
    }

    /// Get a mutable reference to the underlying orchestrator.
    pub fn orchestrator_mut(&mut self) -> &mut Orchestrator {
        &mut self.orchestrator
    }

    // =========================================================================
    // Agent Management
    // =========================================================================

    /// Spawn a new agent with the given capabilities.
    pub fn spawn_agent(&mut self, capabilities: &[Capability]) -> OrchestratorResult<AgentId> {
        let agent_id = self.orchestrator.spawn_agent(capabilities)?;
        if let Some(ref callback) = self.completion_callback {
            callback.on_agent_spawned(agent_id);
        }
        Ok(agent_id)
    }

    /// Get the number of idle agents.
    pub fn idle_agent_count(&self) -> usize {
        self.orchestrator.idle_agents().count()
    }

    /// Get the number of executing agents.
    pub fn executing_agent_count(&self) -> usize {
        self.orchestrator
            .agents()
            .filter(|a| a.state == AgentState::Executing)
            .count()
    }

    // =========================================================================
    // Command Management
    // =========================================================================

    /// Queue a command for execution.
    pub fn queue_command(&mut self, command: Command) -> OrchestratorResult<CommandId> {
        let cmd_id = self.orchestrator.queue_command(command)?;
        if let Some(ref callback) = self.completion_callback {
            callback.on_command_queued(cmd_id);
        }
        Ok(cmd_id)
    }

    /// Approve a command for execution.
    pub fn approve_command(&mut self, command_id: CommandId) -> OrchestratorResult<()> {
        self.orchestrator.approve_command(command_id)
    }

    /// Request approval for a command.
    pub fn request_approval(
        &mut self,
        agent_id: AgentId,
        command_id: CommandId,
    ) -> OrchestratorResult<ApprovalRequestId> {
        self.orchestrator.request_approval(agent_id, command_id)
    }

    /// Approve a pending approval request.
    pub fn approve_request(&mut self, request_id: ApprovalRequestId) -> OrchestratorResult<()> {
        self.orchestrator
            .approve_request(request_id)
            .map_err(|e| OrchestratorError::InvalidStateTransition(e.to_string()))
    }

    /// Reject a pending approval request.
    pub fn reject_request(&mut self, request_id: ApprovalRequestId) -> OrchestratorResult<()> {
        self.orchestrator
            .reject_request(request_id)
            .map_err(|e| OrchestratorError::InvalidStateTransition(e.to_string()))
    }

    // =========================================================================
    // Execution Management
    // =========================================================================

    /// Manually assign a command to an agent.
    pub fn assign_command(
        &mut self,
        agent_id: AgentId,
        command_id: CommandId,
    ) -> OrchestratorResult<()> {
        self.orchestrator.assign_command(agent_id, command_id)?;
        if let Some(ref callback) = self.completion_callback {
            callback.on_command_assigned(command_id, agent_id);
        }
        Ok(())
    }

    /// Manually begin execution for an agent.
    pub fn begin_execution(&mut self, agent_id: AgentId) -> OrchestratorResult<ExecutionId> {
        let command_id = self
            .orchestrator
            .get_agent(agent_id)
            .ok_or(OrchestratorError::AgentNotFound(agent_id))?
            .current_command_id
            .ok_or(OrchestratorError::InvalidStateTransition(
                "No command assigned".to_string(),
            ))?;

        let exec_id = self.orchestrator.begin_execution(agent_id)?;

        if let Some(ref callback) = self.completion_callback {
            callback.on_execution_started(exec_id, agent_id, command_id);
        }

        Ok(exec_id)
    }

    /// Get an execution by ID.
    pub fn get_execution(&self, exec_id: ExecutionId) -> Option<&Execution> {
        self.orchestrator.get_execution(exec_id)
    }

    /// Get the number of active (running) executions.
    pub fn active_execution_count(&self) -> usize {
        self.orchestrator.active_execution_count()
    }

    /// Get recent completion records.
    pub fn recent_completions(&self) -> &[CompletionRecord] {
        &self.recent_completions
    }

    // =========================================================================
    // Runtime Tick
    // =========================================================================

    /// Perform one tick of the runtime loop.
    ///
    /// This method:
    /// 1. Polls all running executions for output and completion
    /// 2. Processes approval timeouts (if interval elapsed)
    /// 3. Auto-assigns commands to agents (if enabled)
    /// 4. Auto-starts executions (if enabled)
    ///
    /// Returns a summary of what happened during the tick.
    ///
    /// ## Scheduling
    ///
    /// Call this method periodically from your main loop. The interval
    /// depends on your latency requirements:
    /// - 1-10ms for interactive use
    /// - 50-100ms for batch processing
    ///
    /// ## Example
    ///
    /// ```rust,ignore
    /// loop {
    ///     let result = runtime.tick();
    ///     if result.completions > 0 {
    ///         // Handle completions
    ///     }
    ///     if !result.had_activity() {
    ///         // No work done, can sleep longer
    ///         std::thread::sleep(Duration::from_millis(50));
    ///     } else {
    ///         // Active, poll more frequently
    ///         std::thread::sleep(Duration::from_millis(5));
    ///     }
    /// }
    /// ```
    pub fn tick(&mut self) -> TickResult {
        // 1. Poll executions for output and completion
        let completions = self.poll_completions();

        // 2. Process approval timeouts
        let approval_timeouts =
            if self.last_timeout_check.elapsed() >= self.config.approval_timeout_interval {
                let timeouts = self.orchestrator.process_approval_timeouts();
                self.last_timeout_check = Instant::now();
                timeouts
            } else {
                0
            };

        // 3. Auto-assign (if enabled)
        let assignments = if self.config.auto_assign {
            self.orchestrator.auto_assign()
        } else {
            0
        };

        // 4. Auto-execute (if enabled)
        let executions_started = if self.config.auto_execute {
            self.auto_execute_with_callback()
        } else {
            0
        };

        TickResult {
            assignments,
            executions_started,
            completions,
            approval_timeouts,
            errors: Vec::new(),
        }
    }

    /// Poll for completion and invoke callbacks.
    fn poll_completions(&mut self) -> usize {
        // We need to track completions separately since poll_executions
        // already transitions state. We'll check before and after.
        let running_before: Vec<_> = self
            .orchestrator
            .running_executions()
            .map(|e| (e.id, e.agent_id, e.command_id))
            .collect();

        let completed_count = self.orchestrator.poll_executions();

        if completed_count > 0 {
            // Find which executions completed
            let still_running: std::collections::HashSet<_> = self
                .orchestrator
                .running_executions()
                .map(|e| e.id)
                .collect();

            for (exec_id, agent_id, command_id) in running_before {
                if !still_running.contains(&exec_id) {
                    // This execution completed
                    if let Some(exec) = self.orchestrator.get_execution(exec_id) {
                        let (exit_code, success) = match exec.state {
                            ExecutionState::Succeeded => (exec.exit_code.unwrap_or(0), true),
                            ExecutionState::Failed => (exec.exit_code.unwrap_or(-1), false),
                            _ => continue,
                        };

                        // Record completion
                        let record = CompletionRecord {
                            exec_id,
                            agent_id,
                            command_id,
                            exit_code,
                            success,
                            completed_at: Instant::now(),
                        };
                        self.recent_completions.push(record);

                        // Trim if needed
                        if self.recent_completions.len() > self.max_recent_completions {
                            self.recent_completions.remove(0);
                        }

                        // Invoke callback
                        if let Some(ref callback) = self.completion_callback {
                            callback
                                .on_completion(exec_id, exit_code, success, agent_id, command_id);
                        }
                    }
                }
            }
        }

        completed_count
    }

    /// Auto-execute with callbacks.
    fn auto_execute_with_callback(&mut self) -> usize {
        // Collect assigned agents
        let assigned: Vec<_> = self
            .orchestrator
            .agents()
            .filter(|a| a.state == AgentState::Assigned)
            .map(|a| (a.id, a.current_command_id))
            .take(self.config.max_executions_per_tick)
            .collect();

        let mut started = 0;
        for (agent_id, command_id) in assigned {
            match self.orchestrator.begin_execution(agent_id) {
                Ok(exec_id) => {
                    started += 1;
                    if let Some(ref callback) = self.completion_callback {
                        if let Some(cmd_id) = command_id {
                            callback.on_execution_started(exec_id, agent_id, cmd_id);
                        }
                    }
                }
                Err(e) => {
                    if let Some(ref callback) = self.completion_callback {
                        if let Some(cmd_id) = command_id {
                            callback.on_execution_failed(agent_id, cmd_id, &e.to_string());
                        }
                    }
                }
            }
        }

        started
    }

    // =========================================================================
    // Convenience Methods
    // =========================================================================

    /// Submit a command and wait for assignment.
    ///
    /// This is a convenience method that:
    /// 1. Queues the command
    /// 2. Runs ticks until the command is assigned
    ///
    /// Returns the assigned agent ID, or an error if assignment fails.
    ///
    /// # Arguments
    ///
    /// * `command` - The command to submit
    /// * `max_ticks` - Maximum ticks to wait for assignment
    pub fn submit_and_wait_for_assignment(
        &mut self,
        command: Command,
        max_ticks: usize,
    ) -> OrchestratorResult<(CommandId, AgentId)> {
        let cmd_id = self.queue_command(command)?;

        for _ in 0..max_ticks {
            self.tick();

            // Check if command is assigned
            for agent in self.orchestrator.agents() {
                if agent.current_command_id == Some(cmd_id) {
                    return Ok((cmd_id, agent.id));
                }
            }
        }

        Err(OrchestratorError::NoCapableAgents)
    }

    /// Run until all executions complete.
    ///
    /// This is useful for testing or batch processing.
    ///
    /// # Arguments
    ///
    /// * `max_ticks` - Maximum ticks before giving up
    ///
    /// # Returns
    ///
    /// Number of completions, or error if max ticks exceeded with running executions.
    pub fn run_until_complete(&mut self, max_ticks: usize) -> OrchestratorResult<usize> {
        let mut total_completions = 0;

        for _ in 0..max_ticks {
            let result = self.tick();
            total_completions += result.completions;

            if self.active_execution_count() == 0 {
                return Ok(total_completions);
            }
        }

        if self.active_execution_count() > 0 {
            Err(OrchestratorError::InvalidStateTransition(
                "Max ticks exceeded with running executions".to_string(),
            ))
        } else {
            Ok(total_completions)
        }
    }

    /// Reset an agent to idle state (for reuse after completion).
    pub fn reset_agent(&mut self, agent_id: AgentId) -> OrchestratorResult<()> {
        self.orchestrator.reset_agent(agent_id)
    }

    /// Clean up old completed executions.
    pub fn cleanup(&mut self, max_age: Duration) {
        self.orchestrator.cleanup(max_age);
    }

    // =========================================================================
    // Stats and Monitoring
    // =========================================================================

    /// Get runtime statistics.
    pub fn stats(&self) -> RuntimeStats {
        let (terminals_available, terminals_in_use) = self.orchestrator.terminal_stats();
        RuntimeStats {
            idle_agents: self.idle_agent_count(),
            executing_agents: self.executing_agent_count(),
            active_executions: self.active_execution_count(),
            pending_approvals: self.orchestrator.pending_approval_count(),
            terminals_available,
            terminals_in_use,
            total_completions: self.recent_completions.len(),
        }
    }
}

/// Runtime statistics.
#[derive(Debug, Clone, Default)]
pub struct RuntimeStats {
    /// Number of idle agents.
    pub idle_agents: usize,
    /// Number of executing agents.
    pub executing_agents: usize,
    /// Number of active executions.
    pub active_executions: usize,
    /// Number of pending approval requests.
    pub pending_approvals: usize,
    /// Number of available terminals.
    pub terminals_available: usize,
    /// Number of terminals in use.
    pub terminals_in_use: usize,
    /// Total completions (from recent history).
    pub total_completions: usize,
}

impl std::fmt::Debug for AgentRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRuntime")
            .field("orchestrator", &self.orchestrator)
            .field("auto_assign", &self.config.auto_assign)
            .field("auto_execute", &self.config.auto_execute)
            .field("recent_completions", &self.recent_completions.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{Command, CommandType};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Test callback that tracks invocations.
    struct TestCallback {
        completions: AtomicUsize,
        executions_started: AtomicUsize,
        agents_spawned: AtomicUsize,
        commands_queued: AtomicUsize,
    }

    impl TestCallback {
        fn new() -> Self {
            Self {
                completions: AtomicUsize::new(0),
                executions_started: AtomicUsize::new(0),
                agents_spawned: AtomicUsize::new(0),
                commands_queued: AtomicUsize::new(0),
            }
        }
    }

    impl CompletionCallback for TestCallback {
        fn on_completion(
            &self,
            _exec_id: ExecutionId,
            _exit_code: i32,
            _success: bool,
            _agent_id: AgentId,
            _command_id: CommandId,
        ) {
            self.completions.fetch_add(1, Ordering::SeqCst);
        }

        fn on_agent_spawned(&self, _agent_id: AgentId) {
            self.agents_spawned.fetch_add(1, Ordering::SeqCst);
        }

        fn on_command_queued(&self, _command_id: CommandId) {
            self.commands_queued.fetch_add(1, Ordering::SeqCst);
        }

        fn on_execution_started(
            &self,
            _exec_id: ExecutionId,
            _agent_id: AgentId,
            _command_id: CommandId,
        ) {
            self.executions_started.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn create_runtime() -> AgentRuntime {
        let config = RuntimeConfig {
            orchestrator: OrchestratorConfig {
                max_agents: 5,
                max_terminals: 3,
                max_queue_size: 10,
                max_executions: 3,
            },
            ..Default::default()
        };
        AgentRuntime::new(config)
    }

    #[test]
    fn test_runtime_creation() {
        let runtime = create_runtime();
        assert_eq!(runtime.idle_agent_count(), 0);
        assert_eq!(runtime.active_execution_count(), 0);
    }

    #[test]
    fn test_spawn_agent() {
        let mut runtime = create_runtime();
        let agent_id = runtime.spawn_agent(&[Capability::Shell]).unwrap();
        assert_eq!(runtime.idle_agent_count(), 1);
        assert!(runtime.orchestrator().get_agent(agent_id).is_some());
    }

    #[test]
    fn test_queue_command() {
        let mut runtime = create_runtime();
        let cmd = Command::shell(CommandId(0), "echo hello");
        let cmd_id = runtime.queue_command(cmd).unwrap();
        assert!(runtime.orchestrator().get_command(cmd_id).is_some());
    }

    #[test]
    fn test_tick_auto_assign() {
        let mut runtime = create_runtime();
        runtime.spawn_agent(&[Capability::Shell]).unwrap();
        runtime
            .queue_command(Command::shell(CommandId(0), "echo hello"))
            .unwrap();

        let result = runtime.tick();
        assert_eq!(result.assignments, 1);
        assert_eq!(result.executions_started, 1);
    }

    #[test]
    fn test_tick_no_auto_assign() {
        let config = RuntimeConfig {
            auto_assign: false,
            auto_execute: false,
            ..Default::default()
        };
        let mut runtime = AgentRuntime::new(config);
        runtime.spawn_agent(&[Capability::Shell]).unwrap();
        runtime
            .queue_command(Command::shell(CommandId(0), "echo hello"))
            .unwrap();

        let result = runtime.tick();
        assert_eq!(result.assignments, 0);
        assert_eq!(result.executions_started, 0);
    }

    #[test]
    fn test_callback_invocations() {
        let callback = Arc::new(TestCallback::new());
        let mut runtime = create_runtime();
        runtime.set_completion_callback(Box::new(callback.clone()));

        // Spawn agent - should trigger callback
        runtime.spawn_agent(&[Capability::Shell]).unwrap();
        assert_eq!(callback.agents_spawned.load(Ordering::SeqCst), 1);

        // Queue command - should trigger callback
        runtime
            .queue_command(Command::shell(CommandId(0), "echo hello"))
            .unwrap();
        assert_eq!(callback.commands_queued.load(Ordering::SeqCst), 1);

        // Tick should start execution and trigger callback
        runtime.tick();
        assert_eq!(callback.executions_started.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_tick_result_activity() {
        let result = TickResult {
            assignments: 1,
            ..Default::default()
        };
        assert!(result.had_activity());

        let result = TickResult::default();
        assert!(!result.had_activity());
    }

    #[test]
    fn test_runtime_stats() {
        let mut runtime = create_runtime();
        runtime.spawn_agent(&[Capability::Shell]).unwrap();

        let stats = runtime.stats();
        assert_eq!(stats.idle_agents, 1);
        assert_eq!(stats.executing_agents, 0);
        assert_eq!(stats.terminals_available, 3);
    }

    #[test]
    fn test_manual_lifecycle() {
        let config = RuntimeConfig {
            auto_assign: false,
            auto_execute: false,
            ..Default::default()
        };
        let mut runtime = AgentRuntime::new(config);

        // Spawn agent
        let agent_id = runtime.spawn_agent(&[Capability::Shell]).unwrap();

        // Queue command
        let cmd = Command::shell(CommandId(0), "echo hello");
        let cmd_id = runtime.queue_command(cmd).unwrap();

        // Manually assign
        runtime.assign_command(agent_id, cmd_id).unwrap();

        // Manually start execution
        let exec_id = runtime.begin_execution(agent_id).unwrap();
        assert!(runtime.get_execution(exec_id).is_some());
        assert_eq!(runtime.active_execution_count(), 1);
    }

    #[test]
    fn test_submit_and_wait_for_assignment() {
        let mut runtime = create_runtime();
        runtime.spawn_agent(&[Capability::Shell]).unwrap();

        let cmd = Command::shell(CommandId(0), "echo hello");
        let (cmd_id, agent_id) = runtime.submit_and_wait_for_assignment(cmd, 10).unwrap();

        assert!(runtime.orchestrator().get_command(cmd_id).is_some());
        assert!(runtime.orchestrator().get_agent(agent_id).is_some());
    }

    #[test]
    fn test_submit_no_capable_agents() {
        let mut runtime = create_runtime();
        // No agents spawned

        let cmd = Command::shell(CommandId(0), "echo hello");
        let result = runtime.submit_and_wait_for_assignment(cmd, 5);

        assert!(matches!(result, Err(OrchestratorError::NoCapableAgents)));
    }

    #[test]
    fn test_approval_workflow() {
        let mut runtime = create_runtime();
        let agent_id = runtime.spawn_agent(&[Capability::Shell]).unwrap();

        // Queue unapproved command
        let cmd = Command::builder(CommandType::Shell)
            .payload("echo hello")
            .build(CommandId(0));
        let cmd_id = runtime.queue_command(cmd).unwrap();

        // Request approval
        let req_id = runtime.request_approval(agent_id, cmd_id).unwrap();

        // Approve
        runtime.approve_request(req_id).unwrap();

        // Now approve the command itself
        runtime.approve_command(cmd_id).unwrap();

        // Should be able to assign now
        runtime.assign_command(agent_id, cmd_id).unwrap();
    }

    #[test]
    fn test_null_completion_callback() {
        let callback = NullCompletionCallback;
        // Should not panic
        callback.on_completion(ExecutionId(0), 0, true, AgentId(0), CommandId(0));
        callback.on_agent_spawned(AgentId(0));
        callback.on_command_queued(CommandId(0));
        callback.on_command_assigned(CommandId(0), AgentId(0));
        callback.on_execution_started(ExecutionId(0), AgentId(0), CommandId(0));
        callback.on_execution_failed(AgentId(0), CommandId(0), "error");
    }

    #[test]
    fn test_runtime_config_default() {
        let config = RuntimeConfig::default();
        assert!(config.auto_assign);
        assert!(config.auto_execute);
        assert_eq!(config.max_executions_per_tick, 10);
    }

    #[test]
    fn test_completion_record() {
        let record = CompletionRecord {
            exec_id: ExecutionId(1),
            agent_id: AgentId(2),
            command_id: CommandId(3),
            exit_code: 0,
            success: true,
            completed_at: Instant::now(),
        };
        assert!(record.success);
        assert_eq!(record.exit_code, 0);
    }

    #[test]
    fn test_runtime_cleanup() {
        let mut runtime = create_runtime();
        runtime.cleanup(Duration::from_secs(60));
        // Should not panic
    }

    #[test]
    fn test_reset_agent() {
        let mut runtime = create_runtime();
        let agent_id = runtime.spawn_agent(&[Capability::Shell]).unwrap();
        runtime
            .queue_command(Command::shell(CommandId(0), "echo hello"))
            .unwrap();

        // Run to get agent executing
        runtime.tick();

        // Complete execution manually
        runtime
            .orchestrator_mut()
            .complete_execution(agent_id, 0)
            .unwrap();

        // Reset agent
        runtime.reset_agent(agent_id).unwrap();
        assert_eq!(runtime.idle_agent_count(), 1);
    }

    #[test]
    fn test_has_domain_support() {
        let runtime = create_runtime();
        assert!(!runtime.has_domain_support());
    }

    // =========================================================================
    // Domain Integration Tests
    // =========================================================================

    mod domain_tests {
        use super::*;
        use crate::domain::{
            Domain, DomainId, DomainResult, DomainState, DomainType, Pane, PaneId, SpawnConfig,
        };
        use std::sync::Mutex;

        /// Controllable mock pane for testing completion detection.
        struct ControllableMockPane {
            id: PaneId,
            domain_id: DomainId,
            alive: Arc<Mutex<bool>>,
            exit_code: Arc<Mutex<Option<i32>>>,
            output: Arc<Mutex<Vec<u8>>>,
        }

        impl ControllableMockPane {
            fn new(domain_id: DomainId) -> Self {
                Self {
                    id: PaneId::new(),
                    domain_id,
                    alive: Arc::new(Mutex::new(true)),
                    exit_code: Arc::new(Mutex::new(None)),
                    output: Arc::new(Mutex::new(Vec::new())),
                }
            }

            fn simulate_exit(&self, code: i32) {
                *self.alive.lock().unwrap() = false;
                *self.exit_code.lock().unwrap() = Some(code);
            }

            fn add_output(&self, data: &[u8]) {
                self.output.lock().unwrap().extend_from_slice(data);
            }
        }

        impl Pane for ControllableMockPane {
            fn pane_id(&self) -> PaneId {
                self.id
            }

            fn domain_id(&self) -> DomainId {
                self.domain_id
            }

            fn size(&self) -> (u16, u16) {
                (80, 24)
            }

            fn resize(&self, _cols: u16, _rows: u16) -> DomainResult<()> {
                Ok(())
            }

            fn write(&self, _data: &[u8]) -> DomainResult<usize> {
                Ok(0)
            }

            fn read(&self, buf: &mut [u8]) -> DomainResult<usize> {
                let mut output = self.output.lock().unwrap();
                if output.is_empty() {
                    return Ok(0);
                }
                let len = output.len().min(buf.len());
                buf[..len].copy_from_slice(&output[..len]);
                output.drain(..len);
                Ok(len)
            }

            fn is_alive(&self) -> bool {
                *self.alive.lock().unwrap()
            }

            fn exit_status(&self) -> Option<i32> {
                *self.exit_code.lock().unwrap()
            }

            fn kill(&self) -> DomainResult<()> {
                self.simulate_exit(-9);
                Ok(())
            }
        }

        /// Mock domain that spawns controllable panes.
        struct ControllableMockDomain {
            id: DomainId,
            name: String,
            spawned_panes: Mutex<Vec<Arc<ControllableMockPane>>>,
        }

        impl ControllableMockDomain {
            fn new(name: &str) -> Self {
                Self {
                    id: DomainId::new(),
                    name: name.to_string(),
                    spawned_panes: Mutex::new(Vec::new()),
                }
            }

            fn get_last_pane(&self) -> Option<Arc<ControllableMockPane>> {
                self.spawned_panes.lock().unwrap().last().cloned()
            }
        }

        impl Domain for ControllableMockDomain {
            fn domain_id(&self) -> DomainId {
                self.id
            }

            fn domain_name(&self) -> &str {
                &self.name
            }

            fn domain_type(&self) -> DomainType {
                DomainType::Local
            }

            fn state(&self) -> DomainState {
                DomainState::Attached
            }

            fn detachable(&self) -> bool {
                false
            }

            fn attach(&self) -> DomainResult<()> {
                Ok(())
            }

            fn detach(&self) -> DomainResult<()> {
                Ok(())
            }

            fn spawn_pane(
                &self,
                _cols: u16,
                _rows: u16,
                _config: SpawnConfig,
            ) -> DomainResult<Arc<dyn Pane>> {
                let pane = Arc::new(ControllableMockPane::new(self.id));
                self.spawned_panes.lock().unwrap().push(Arc::clone(&pane));
                Ok(pane)
            }

            fn get_pane(&self, _id: PaneId) -> Option<Arc<dyn Pane>> {
                None
            }

            fn list_panes(&self) -> Vec<Arc<dyn Pane>> {
                vec![]
            }

            fn remove_pane(&self, _id: PaneId) -> Option<Arc<dyn Pane>> {
                None
            }
        }

        #[test]
        fn test_runtime_with_domain_completion() {
            let callback = Arc::new(TestCallback::new());
            let mut runtime = create_runtime();
            runtime.set_completion_callback(Box::new(callback.clone()));

            // Set up domain
            let domain = Arc::new(ControllableMockDomain::new("test-domain"));
            runtime.set_default_domain(domain.clone());
            assert!(runtime.has_domain_support());

            // Spawn agent and queue command
            runtime.spawn_agent(&[Capability::Shell]).unwrap();
            runtime
                .queue_command(Command::shell(CommandId(0), "echo hello"))
                .unwrap();

            // Tick to start execution
            let result = runtime.tick();
            assert_eq!(result.executions_started, 1);
            assert_eq!(callback.executions_started.load(Ordering::SeqCst), 1);

            // Simulate output and exit
            let pane = domain.get_last_pane().unwrap();
            pane.add_output(b"hello\n");
            pane.simulate_exit(0);

            // Tick to detect completion
            let result = runtime.tick();
            assert_eq!(result.completions, 1);
            assert_eq!(callback.completions.load(Ordering::SeqCst), 1);

            // Check recent completions
            assert_eq!(runtime.recent_completions().len(), 1);
            assert!(runtime.recent_completions()[0].success);
        }

        #[test]
        fn test_runtime_with_domain_failure() {
            let callback = Arc::new(TestCallback::new());
            let mut runtime = create_runtime();
            runtime.set_completion_callback(Box::new(callback.clone()));

            let domain = Arc::new(ControllableMockDomain::new("test-domain"));
            runtime.set_default_domain(domain.clone());

            runtime.spawn_agent(&[Capability::Shell]).unwrap();
            runtime
                .queue_command(Command::shell(CommandId(0), "false"))
                .unwrap();

            // Start execution
            runtime.tick();

            // Simulate failed exit
            let pane = domain.get_last_pane().unwrap();
            pane.simulate_exit(1);

            // Detect completion
            let result = runtime.tick();
            assert_eq!(result.completions, 1);

            // Check failure was recorded
            assert!(!runtime.recent_completions()[0].success);
            assert_eq!(runtime.recent_completions()[0].exit_code, 1);
        }

        #[test]
        fn test_run_until_complete_with_domain() {
            let domain = Arc::new(ControllableMockDomain::new("test-domain"));
            let mut runtime = create_runtime();
            runtime.set_default_domain(domain.clone());

            runtime.spawn_agent(&[Capability::Shell]).unwrap();
            runtime
                .queue_command(Command::shell(CommandId(0), "echo hello"))
                .unwrap();

            // Start execution first
            runtime.tick();

            // Simulate exit
            let pane = domain.get_last_pane().unwrap();
            pane.simulate_exit(0);

            // Run until complete
            let completions = runtime.run_until_complete(100).unwrap();
            assert_eq!(completions, 1);
            assert_eq!(runtime.active_execution_count(), 0);
        }

        #[test]
        fn test_multiple_executions_with_domain() {
            let domain = Arc::new(ControllableMockDomain::new("test-domain"));
            let mut runtime = create_runtime();
            runtime.set_default_domain(domain.clone());

            // Spawn 2 agents
            runtime.spawn_agent(&[Capability::Shell]).unwrap();
            runtime.spawn_agent(&[Capability::Shell]).unwrap();

            // Queue 2 commands
            runtime
                .queue_command(Command::shell(CommandId(0), "cmd1"))
                .unwrap();
            runtime
                .queue_command(Command::shell(CommandId(0), "cmd2"))
                .unwrap();

            // Start executions
            runtime.tick();
            assert_eq!(runtime.active_execution_count(), 2);

            // Simulate exits
            let panes: Vec<_> = domain.spawned_panes.lock().unwrap().clone();
            panes[0].simulate_exit(0);
            panes[1].simulate_exit(0);

            // Detect completions
            let result = runtime.tick();
            assert_eq!(result.completions, 2);
            assert_eq!(runtime.active_execution_count(), 0);
        }
    }
}
