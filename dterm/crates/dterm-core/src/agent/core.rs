//! Agent types and state machine.
//!
//! Implements the Agent record from AgentOrchestration.tla:
//! ```text
//! Agent == [
//!     id: Nat,
//!     state: AgentStates,
//!     capabilities: SUBSET Capabilities,
//!     currentCommandId: Nat ∪ {-1},
//!     currentExecutionId: Nat ∪ {-1}
//! ]
//! ```

use std::collections::HashSet;
use std::fmt;

use super::{CommandId, ExecutionId};

/// Unique identifier for an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AgentId(pub u64);

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Agent({})", self.0)
    }
}

/// Agent capabilities for command routing.
///
/// Maps to `Capabilities` constant in TLA+ spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Shell command execution
    Shell,
    /// File system operations
    File,
    /// Network operations
    Net,
    /// System administration
    Admin,
    /// Git operations
    Git,
    /// Package management
    Package,
    /// Container operations
    Container,
    /// Database operations
    Database,
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Capability::Shell => "shell",
            Capability::File => "file",
            Capability::Net => "net",
            Capability::Admin => "admin",
            Capability::Git => "git",
            Capability::Package => "package",
            Capability::Container => "container",
            Capability::Database => "database",
        };
        write!(f, "{}", name)
    }
}

/// Agent lifecycle states.
///
/// Maps to `AgentStates` in TLA+ spec:
/// ```text
/// AgentStates == {"Idle", "Assigned", "Executing", "Completed", "Failed", "Cancelled"}
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentState {
    /// Agent is idle, ready for assignment
    Idle,
    /// Agent has been assigned a command but not yet executing
    Assigned,
    /// Agent is actively executing a command
    Executing,
    /// Agent completed command successfully
    Completed,
    /// Agent failed to complete command
    Failed,
    /// Agent execution was cancelled
    Cancelled,
}

impl AgentState {
    /// Returns true if the agent can be assigned a new command.
    pub fn can_accept_command(&self) -> bool {
        matches!(self, AgentState::Idle)
    }

    /// Returns true if the agent is in a terminal state (completed, failed, or cancelled).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            AgentState::Completed | AgentState::Failed | AgentState::Cancelled
        )
    }

    /// Returns true if the agent can be reset to idle.
    pub fn can_reset(&self) -> bool {
        self.is_terminal()
    }
}

impl fmt::Display for AgentState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            AgentState::Idle => "Idle",
            AgentState::Assigned => "Assigned",
            AgentState::Executing => "Executing",
            AgentState::Completed => "Completed",
            AgentState::Failed => "Failed",
            AgentState::Cancelled => "Cancelled",
        };
        write!(f, "{}", name)
    }
}

/// An AI agent that can execute terminal commands.
///
/// Implements the Agent record from the TLA+ specification.
#[derive(Debug, Clone)]
pub struct Agent {
    /// Unique identifier
    pub id: AgentId,
    /// Current lifecycle state
    pub state: AgentState,
    /// Set of capabilities this agent has
    pub capabilities: HashSet<Capability>,
    /// Currently assigned command (None = -1 in TLA+)
    pub current_command_id: Option<CommandId>,
    /// Currently running execution (None = -1 in TLA+)
    pub current_execution_id: Option<ExecutionId>,
}

impl Agent {
    /// Create a new agent with the given capabilities.
    ///
    /// # Panics
    ///
    /// Panics if `capabilities` is empty (TLA+ precondition: `caps # {}`).
    pub fn new(id: AgentId, capabilities: HashSet<Capability>) -> Self {
        assert!(
            !capabilities.is_empty(),
            "Agent must have at least one capability"
        );
        Self {
            id,
            state: AgentState::Idle,
            capabilities,
            current_command_id: None,
            current_execution_id: None,
        }
    }

    /// Check if this agent has all the required capabilities.
    ///
    /// Implements `CanHandle` helper from TLA+:
    /// ```text
    /// CanHandle(agentId, cmd) ==
    ///     cmd.requiredCapabilities ⊆ agents[agentId].capabilities
    /// ```
    pub fn has_capabilities(&self, required: &HashSet<Capability>) -> bool {
        required.is_subset(&self.capabilities)
    }

    /// Assign a command to this agent.
    ///
    /// # Preconditions (enforced by TLA+ spec)
    /// - Agent must be in Idle state
    /// - Agent must have required capabilities (checked externally)
    ///
    /// # Returns
    /// - `Ok(())` if assignment succeeded
    /// - `Err(message)` if preconditions violated
    pub fn assign(&mut self, command_id: CommandId) -> Result<(), &'static str> {
        if self.state != AgentState::Idle {
            return Err("Agent must be Idle to accept command assignment");
        }
        self.state = AgentState::Assigned;
        self.current_command_id = Some(command_id);
        Ok(())
    }

    /// Begin execution of the assigned command.
    ///
    /// # Preconditions
    /// - Agent must be in Assigned state
    /// - Must have a command assigned
    ///
    /// # Returns
    /// - `Ok(())` if transition succeeded
    /// - `Err(message)` if preconditions violated
    pub fn begin_execution(&mut self, execution_id: ExecutionId) -> Result<(), &'static str> {
        if self.state != AgentState::Assigned {
            return Err("Agent must be Assigned to begin execution");
        }
        if self.current_command_id.is_none() {
            return Err("Agent must have command assigned before execution");
        }
        self.state = AgentState::Executing;
        self.current_execution_id = Some(execution_id);
        Ok(())
    }

    /// Complete execution successfully.
    ///
    /// # Preconditions
    /// - Agent must be in Executing state
    pub fn complete(&mut self) -> Result<(), &'static str> {
        if self.state != AgentState::Executing {
            return Err("Agent must be Executing to complete");
        }
        self.state = AgentState::Completed;
        self.current_command_id = None;
        self.current_execution_id = None;
        Ok(())
    }

    /// Mark execution as failed.
    ///
    /// # Preconditions
    /// - Agent must be in Executing state
    pub fn fail(&mut self) -> Result<(), &'static str> {
        if self.state != AgentState::Executing {
            return Err("Agent must be Executing to fail");
        }
        self.state = AgentState::Failed;
        self.current_command_id = None;
        self.current_execution_id = None;
        Ok(())
    }

    /// Cancel the current operation.
    ///
    /// # Preconditions
    /// - Agent must be in Assigned or Executing state
    pub fn cancel(&mut self) -> Result<(), &'static str> {
        if !matches!(self.state, AgentState::Assigned | AgentState::Executing) {
            return Err("Agent must be Assigned or Executing to cancel");
        }
        self.state = AgentState::Cancelled;
        self.current_command_id = None;
        self.current_execution_id = None;
        Ok(())
    }

    /// Reset agent to Idle state.
    ///
    /// # Preconditions
    /// - Agent must be in terminal state (Completed, Failed, or Cancelled)
    pub fn reset(&mut self) -> Result<(), &'static str> {
        if !self.state.can_reset() {
            return Err("Agent must be in terminal state (Completed/Failed/Cancelled) to reset");
        }
        self.state = AgentState::Idle;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_agent(caps: &[Capability]) -> Agent {
        Agent::new(AgentId(1), caps.iter().copied().collect())
    }

    #[test]
    fn test_agent_creation() {
        let agent = make_agent(&[Capability::Shell, Capability::File]);
        assert_eq!(agent.state, AgentState::Idle);
        assert!(agent.capabilities.contains(&Capability::Shell));
        assert!(agent.capabilities.contains(&Capability::File));
        assert!(!agent.capabilities.contains(&Capability::Net));
    }

    #[test]
    #[should_panic(expected = "at least one capability")]
    fn test_agent_requires_capabilities() {
        Agent::new(AgentId(1), HashSet::new());
    }

    #[test]
    fn test_agent_capability_check() {
        let agent = make_agent(&[Capability::Shell, Capability::File]);

        // Has shell
        let required: HashSet<_> = [Capability::Shell].into_iter().collect();
        assert!(agent.has_capabilities(&required));

        // Has shell and file
        let required: HashSet<_> = [Capability::Shell, Capability::File].into_iter().collect();
        assert!(agent.has_capabilities(&required));

        // Does not have net
        let required: HashSet<_> = [Capability::Net].into_iter().collect();
        assert!(!agent.has_capabilities(&required));

        // Partial match fails
        let required: HashSet<_> = [Capability::Shell, Capability::Net].into_iter().collect();
        assert!(!agent.has_capabilities(&required));
    }

    #[test]
    fn test_agent_lifecycle_happy_path() {
        let mut agent = make_agent(&[Capability::Shell]);

        // Idle -> Assigned
        assert!(agent.assign(CommandId(1)).is_ok());
        assert_eq!(agent.state, AgentState::Assigned);
        assert_eq!(agent.current_command_id, Some(CommandId(1)));

        // Assigned -> Executing
        assert!(agent.begin_execution(ExecutionId(1)).is_ok());
        assert_eq!(agent.state, AgentState::Executing);
        assert_eq!(agent.current_execution_id, Some(ExecutionId(1)));

        // Executing -> Completed
        assert!(agent.complete().is_ok());
        assert_eq!(agent.state, AgentState::Completed);
        assert_eq!(agent.current_command_id, None);
        assert_eq!(agent.current_execution_id, None);

        // Completed -> Idle (reset)
        assert!(agent.reset().is_ok());
        assert_eq!(agent.state, AgentState::Idle);
    }

    #[test]
    fn test_agent_failure_path() {
        let mut agent = make_agent(&[Capability::Shell]);
        agent.assign(CommandId(1)).unwrap();
        agent.begin_execution(ExecutionId(1)).unwrap();

        assert!(agent.fail().is_ok());
        assert_eq!(agent.state, AgentState::Failed);

        assert!(agent.reset().is_ok());
        assert_eq!(agent.state, AgentState::Idle);
    }

    #[test]
    fn test_agent_cancel_from_assigned() {
        let mut agent = make_agent(&[Capability::Shell]);
        agent.assign(CommandId(1)).unwrap();

        assert!(agent.cancel().is_ok());
        assert_eq!(agent.state, AgentState::Cancelled);
    }

    #[test]
    fn test_agent_cancel_from_executing() {
        let mut agent = make_agent(&[Capability::Shell]);
        agent.assign(CommandId(1)).unwrap();
        agent.begin_execution(ExecutionId(1)).unwrap();

        assert!(agent.cancel().is_ok());
        assert_eq!(agent.state, AgentState::Cancelled);
    }

    #[test]
    fn test_invalid_state_transitions() {
        let mut agent = make_agent(&[Capability::Shell]);

        // Can't execute without assignment
        assert!(agent.begin_execution(ExecutionId(1)).is_err());

        // Can't complete without executing
        assert!(agent.complete().is_err());

        // Can't fail without executing
        assert!(agent.fail().is_err());

        // Can't cancel from idle
        assert!(agent.cancel().is_err());

        // Can't reset from non-terminal
        assert!(agent.reset().is_err());

        // Can't double-assign
        agent.assign(CommandId(1)).unwrap();
        assert!(agent.assign(CommandId(2)).is_err());
    }
}
