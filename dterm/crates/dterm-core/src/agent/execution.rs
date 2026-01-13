//! Execution tracking and management.
//!
//! Implements the Execution record from AgentOrchestration.tla:
//! ```text
//! Execution == [
//!     id: Nat,
//!     agentId: Nat,
//!     commandId: Nat,
//!     terminalId: Nat,
//!     state: ExecutionStates,
//!     startTime: Nat,
//!     endTime: Nat ∪ {-1}
//! ]
//! ```

use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

use super::{AgentId, CommandId, TerminalSlotId};

/// Unique identifier for an execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExecutionId(pub u64);

impl fmt::Display for ExecutionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Exec({})", self.0)
    }
}

/// Execution lifecycle states.
///
/// Maps to `ExecutionStates` in TLA+ spec:
/// ```text
/// ExecutionStates == {"Running", "Succeeded", "Failed", "Cancelled"}
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExecutionState {
    /// Execution is currently running
    Running,
    /// Execution completed successfully
    Succeeded,
    /// Execution failed
    Failed,
    /// Execution was cancelled
    Cancelled,
}

impl ExecutionState {
    /// Returns true if the execution is still in progress.
    pub fn is_running(&self) -> bool {
        matches!(self, ExecutionState::Running)
    }

    /// Returns true if the execution has terminated.
    pub fn is_terminal(&self) -> bool {
        !self.is_running()
    }

    /// Returns true if the execution completed successfully.
    pub fn is_success(&self) -> bool {
        matches!(self, ExecutionState::Succeeded)
    }
}

impl fmt::Display for ExecutionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            ExecutionState::Running => "Running",
            ExecutionState::Succeeded => "Succeeded",
            ExecutionState::Failed => "Failed",
            ExecutionState::Cancelled => "Cancelled",
        };
        write!(f, "{}", name)
    }
}

/// A single execution of a command by an agent.
///
/// Implements the Execution record from the TLA+ specification.
#[derive(Debug, Clone)]
pub struct Execution {
    /// Unique identifier
    pub id: ExecutionId,
    /// Agent running this execution
    pub agent_id: AgentId,
    /// Command being executed
    pub command_id: CommandId,
    /// Terminal slot allocated for this execution
    pub terminal_id: TerminalSlotId,
    /// Current state
    pub state: ExecutionState,
    /// When execution started
    pub start_time: Instant,
    /// When execution ended (None if still running)
    pub end_time: Option<Instant>,
    /// Exit code if completed
    pub exit_code: Option<i32>,
    /// Error message if failed
    pub error: Option<String>,
    /// Captured stdout
    pub stdout: Vec<u8>,
    /// Captured stderr
    pub stderr: Vec<u8>,
}

impl Execution {
    /// Create a new running execution.
    pub fn new(
        id: ExecutionId,
        agent_id: AgentId,
        command_id: CommandId,
        terminal_id: TerminalSlotId,
    ) -> Self {
        Self {
            id,
            agent_id,
            command_id,
            terminal_id,
            state: ExecutionState::Running,
            start_time: Instant::now(),
            end_time: None,
            exit_code: None,
            error: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
        }
    }

    /// Mark execution as succeeded.
    ///
    /// # Preconditions
    /// - Execution must be in Running state
    pub fn succeed(&mut self, exit_code: i32) -> Result<(), &'static str> {
        if self.state != ExecutionState::Running {
            return Err("Execution must be Running to succeed");
        }
        self.state = ExecutionState::Succeeded;
        self.end_time = Some(Instant::now());
        self.exit_code = Some(exit_code);
        Ok(())
    }

    /// Mark execution as failed.
    ///
    /// # Preconditions
    /// - Execution must be in Running state
    pub fn fail(&mut self, error: impl Into<String>) -> Result<(), &'static str> {
        if self.state != ExecutionState::Running {
            return Err("Execution must be Running to fail");
        }
        self.state = ExecutionState::Failed;
        self.end_time = Some(Instant::now());
        self.error = Some(error.into());
        Ok(())
    }

    /// Mark execution as failed with a specific exit code.
    ///
    /// # Preconditions
    /// - Execution must be in Running state
    pub fn fail_with_exit_code(
        &mut self,
        exit_code: i32,
        error: impl Into<String>,
    ) -> Result<(), &'static str> {
        if self.state != ExecutionState::Running {
            return Err("Execution must be Running to fail");
        }
        self.state = ExecutionState::Failed;
        self.end_time = Some(Instant::now());
        self.exit_code = Some(exit_code);
        self.error = Some(error.into());
        Ok(())
    }

    /// Mark execution as cancelled.
    ///
    /// # Preconditions
    /// - Execution must be in Running state
    pub fn cancel(&mut self) -> Result<(), &'static str> {
        if self.state != ExecutionState::Running {
            return Err("Execution must be Running to cancel");
        }
        self.state = ExecutionState::Cancelled;
        self.end_time = Some(Instant::now());
        Ok(())
    }

    /// Get execution duration.
    pub fn duration(&self) -> Duration {
        let end = self.end_time.unwrap_or_else(Instant::now);
        end.duration_since(self.start_time)
    }

    /// Append to stdout buffer.
    pub fn append_stdout(&mut self, data: &[u8]) {
        self.stdout.extend_from_slice(data);
    }

    /// Append to stderr buffer.
    pub fn append_stderr(&mut self, data: &[u8]) {
        self.stderr.extend_from_slice(data);
    }
}

/// Manager for tracking all executions.
#[derive(Debug)]
pub struct ExecutionManager {
    /// All executions by ID
    executions: HashMap<ExecutionId, Execution>,
    /// Maximum concurrent executions (TLA+: MaxExecutions)
    max_executions: usize,
    /// Next execution ID to assign
    next_id: u64,
}

impl ExecutionManager {
    /// Create a new execution manager.
    pub fn new(max_executions: usize) -> Self {
        Self {
            executions: HashMap::new(),
            max_executions,
            next_id: 0,
        }
    }

    /// Get the number of active (running) executions.
    pub fn active_count(&self) -> usize {
        self.executions
            .values()
            .filter(|e| e.state.is_running())
            .count()
    }

    /// Get total number of executions (including completed).
    pub fn total_count(&self) -> usize {
        self.executions.len()
    }

    /// Check if we can start a new execution.
    pub fn can_start(&self) -> bool {
        self.active_count() < self.max_executions
    }

    /// Generate a new execution ID.
    fn next_execution_id(&mut self) -> ExecutionId {
        let id = ExecutionId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Start a new execution.
    ///
    /// # Returns
    /// - `Ok(ExecutionId)` if execution started
    /// - `Err(&'static str)` if max concurrent executions reached
    pub fn start(
        &mut self,
        agent_id: AgentId,
        command_id: CommandId,
        terminal_id: TerminalSlotId,
    ) -> Result<ExecutionId, &'static str> {
        if !self.can_start() {
            return Err("Maximum concurrent executions reached");
        }
        let id = self.next_execution_id();
        let execution = Execution::new(id, agent_id, command_id, terminal_id);
        self.executions.insert(id, execution);
        Ok(id)
    }

    /// Get an execution by ID.
    pub fn get(&self, id: ExecutionId) -> Option<&Execution> {
        self.executions.get(&id)
    }

    /// Get a mutable reference to an execution.
    pub fn get_mut(&mut self, id: ExecutionId) -> Option<&mut Execution> {
        self.executions.get_mut(&id)
    }

    /// Find execution by agent ID.
    pub fn find_by_agent(&self, agent_id: AgentId) -> Option<&Execution> {
        self.executions
            .values()
            .find(|e| e.agent_id == agent_id && e.state.is_running())
    }

    /// Find execution by terminal slot.
    pub fn find_by_terminal(&self, terminal_id: TerminalSlotId) -> Option<&Execution> {
        self.executions
            .values()
            .find(|e| e.terminal_id == terminal_id && e.state.is_running())
    }

    /// Get all running executions.
    pub fn running(&self) -> impl Iterator<Item = &Execution> {
        self.executions.values().filter(|e| e.state.is_running())
    }

    /// Get all completed executions.
    pub fn completed(&self) -> impl Iterator<Item = &Execution> {
        self.executions.values().filter(|e| e.state.is_terminal())
    }

    /// Clean up old completed executions to save memory.
    ///
    /// Removes executions older than `max_age`.
    pub fn cleanup(&mut self, max_age: Duration) {
        let Some(cutoff) = Instant::now().checked_sub(max_age) else {
            return; // max_age is larger than time since program start
        };
        self.executions.retain(|_, e| {
            // Keep running executions
            if e.state.is_running() {
                return true;
            }
            // Keep completed executions newer than cutoff
            match e.end_time {
                Some(end) => end > cutoff,
                None => true, // Should not happen for completed executions
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_lifecycle() {
        let mut exec = Execution::new(ExecutionId(1), AgentId(1), CommandId(1), TerminalSlotId(1));

        assert_eq!(exec.state, ExecutionState::Running);
        assert!(exec.end_time.is_none());

        exec.succeed(0).unwrap();
        assert_eq!(exec.state, ExecutionState::Succeeded);
        assert!(exec.end_time.is_some());
        assert_eq!(exec.exit_code, Some(0));
    }

    #[test]
    fn test_execution_failure() {
        let mut exec = Execution::new(ExecutionId(1), AgentId(1), CommandId(1), TerminalSlotId(1));

        exec.fail("Something went wrong").unwrap();
        assert_eq!(exec.state, ExecutionState::Failed);
        assert_eq!(exec.error, Some("Something went wrong".to_string()));
    }

    #[test]
    fn test_execution_cancel() {
        let mut exec = Execution::new(ExecutionId(1), AgentId(1), CommandId(1), TerminalSlotId(1));

        exec.cancel().unwrap();
        assert_eq!(exec.state, ExecutionState::Cancelled);
    }

    #[test]
    fn test_double_transition_fails() {
        let mut exec = Execution::new(ExecutionId(1), AgentId(1), CommandId(1), TerminalSlotId(1));

        exec.succeed(0).unwrap();
        assert!(exec.succeed(0).is_err());
        assert!(exec.fail("error").is_err());
        assert!(exec.cancel().is_err());
    }

    #[test]
    fn test_execution_manager() {
        let mut manager = ExecutionManager::new(3);

        assert!(manager.can_start());
        assert_eq!(manager.active_count(), 0);

        // Start 3 executions
        let id1 = manager
            .start(AgentId(1), CommandId(1), TerminalSlotId(1))
            .unwrap();
        let id2 = manager
            .start(AgentId(2), CommandId(2), TerminalSlotId(2))
            .unwrap();
        let id3 = manager
            .start(AgentId(3), CommandId(3), TerminalSlotId(3))
            .unwrap();

        assert_eq!(manager.active_count(), 3);
        assert!(!manager.can_start());

        // Fourth should fail
        assert!(manager
            .start(AgentId(4), CommandId(4), TerminalSlotId(4))
            .is_err());

        // Complete one
        manager.get_mut(id1).unwrap().succeed(0).unwrap();
        assert_eq!(manager.active_count(), 2);
        assert!(manager.can_start());

        // Can find by agent
        assert!(manager.find_by_agent(AgentId(2)).is_some());
        assert!(manager.find_by_agent(AgentId(1)).is_none()); // Completed

        // Can find by terminal
        assert!(manager.find_by_terminal(TerminalSlotId(3)).is_some());

        // Get running and completed
        assert_eq!(manager.running().count(), 2);
        assert_eq!(manager.completed().count(), 1);

        // Verify specific IDs
        assert!(manager.get(id1).is_some());
        assert!(manager.get(id2).is_some());
        assert!(manager.get(id3).is_some());
    }
}
