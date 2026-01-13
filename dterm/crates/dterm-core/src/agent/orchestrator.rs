//! Agent orchestrator - central coordinator for agent execution.
//!
//! Implements the state machine from AgentOrchestration.tla, coordinating:
//! - Agent lifecycle management
//! - Command queue and assignment
//! - Execution tracking
//! - Terminal pool allocation
//!
//! ## Safety Invariants
//!
//! The orchestrator enforces all TLA+ safety invariants:
//! - INV-ORCH-1: No command assigned to multiple agents
//! - INV-ORCH-2: Every execution has an assigned agent
//! - INV-ORCH-3: Terminal exclusivity (one execution per terminal)
//! - INV-ORCH-4: Dependencies respected
//! - INV-ORCH-5: Agent capability matching
//! - INV-ORCH-6: Executing agent has valid terminal
//! - INV-ORCH-7: Only approved commands execute

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use super::approval::{
    Action, ApprovalCallback, ApprovalConfig, ApprovalError, ApprovalManager, ApprovalRequestId,
    ApprovalResult,
};
use super::CommandType;
use super::{
    Agent, AgentId, AgentState, Capability, Command, CommandId, CommandQueue, CommandQueueError,
    Execution, ExecutionId, ExecutionManager, TerminalPool, TerminalSlotId,
};
use crate::domain::{Domain, DomainError, DomainRegistry};

/// Convert a command type to an approval action.
fn command_type_to_action(cmd_type: CommandType) -> Action {
    match cmd_type {
        CommandType::Shell => Action::Shell,
        CommandType::FileOp => Action::FileWrite,
        CommandType::Network => Action::Network,
        CommandType::Git => Action::GitPush,
        CommandType::Package => Action::PackageInstall,
        CommandType::Container => Action::Container,
        CommandType::Database => Action::DatabaseWrite,
        CommandType::Admin => Action::Admin,
    }
}

/// Orchestrator configuration.
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Maximum number of agents
    pub max_agents: usize,
    /// Maximum number of terminal slots
    pub max_terminals: usize,
    /// Maximum command queue size
    pub max_queue_size: usize,
    /// Maximum concurrent executions
    pub max_executions: usize,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_agents: 10,
            max_terminals: 5,
            max_queue_size: 100,
            max_executions: 5,
        }
    }
}

impl OrchestratorConfig {
    /// Create a new configuration with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of agents.
    #[must_use]
    pub fn with_max_agents(mut self, max_agents: usize) -> Self {
        self.max_agents = max_agents;
        self
    }

    /// Set the maximum number of terminal slots.
    #[must_use]
    pub fn with_max_terminals(mut self, max_terminals: usize) -> Self {
        self.max_terminals = max_terminals;
        self
    }

    /// Set the maximum command queue size.
    #[must_use]
    pub fn with_max_queue_size(mut self, max_queue_size: usize) -> Self {
        self.max_queue_size = max_queue_size;
        self
    }

    /// Set the maximum concurrent executions.
    #[must_use]
    pub fn with_max_executions(mut self, max_executions: usize) -> Self {
        self.max_executions = max_executions;
        self
    }
}

/// Orchestrator errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrchestratorError {
    /// Maximum agents reached
    MaxAgentsReached,
    /// Maximum concurrent executions reached
    MaxExecutionsReached,
    /// Agent not found
    AgentNotFound(AgentId),
    /// Command not found
    CommandNotFound(CommandId),
    /// Execution not found
    ExecutionNotFound(ExecutionId),
    /// Command queue full
    QueueFull,
    /// No available terminals
    NoTerminalsAvailable,
    /// No capable agents
    NoCapableAgents,
    /// Invalid state transition
    InvalidStateTransition(String),
    /// Capability mismatch
    CapabilityMismatch,
    /// Dependencies not satisfied
    DependenciesNotSatisfied,
    /// Command not approved
    NotApproved,
    /// Pane spawn failed (Phase 12)
    SpawnFailed(String),
    /// No domain configured (Phase 12)
    NoDomainConfigured,
}

impl fmt::Display for OrchestratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrchestratorError::MaxAgentsReached => write!(f, "Maximum agents reached"),
            OrchestratorError::MaxExecutionsReached => {
                write!(f, "Maximum concurrent executions reached")
            }
            OrchestratorError::AgentNotFound(id) => write!(f, "Agent not found: {}", id),
            OrchestratorError::CommandNotFound(id) => write!(f, "Command not found: {}", id),
            OrchestratorError::ExecutionNotFound(id) => write!(f, "Execution not found: {}", id),
            OrchestratorError::QueueFull => write!(f, "Command queue full"),
            OrchestratorError::NoTerminalsAvailable => write!(f, "No terminal slots available"),
            OrchestratorError::NoCapableAgents => write!(f, "No capable agents available"),
            OrchestratorError::InvalidStateTransition(msg) => write!(f, "Invalid state: {}", msg),
            OrchestratorError::CapabilityMismatch => write!(f, "Agent lacks required capabilities"),
            OrchestratorError::DependenciesNotSatisfied => {
                write!(f, "Command dependencies not satisfied")
            }
            OrchestratorError::NotApproved => write!(f, "Command not approved"),
            OrchestratorError::SpawnFailed(msg) => write!(f, "Pane spawn failed: {}", msg),
            OrchestratorError::NoDomainConfigured => {
                write!(f, "No domain configured for pane spawning")
            }
        }
    }
}

impl std::error::Error for OrchestratorError {}

impl From<DomainError> for OrchestratorError {
    fn from(err: DomainError) -> Self {
        OrchestratorError::SpawnFailed(err.to_string())
    }
}

impl From<ApprovalError> for OrchestratorError {
    fn from(err: ApprovalError) -> Self {
        OrchestratorError::InvalidStateTransition(err.to_string())
    }
}

impl From<CommandQueueError> for OrchestratorError {
    fn from(err: CommandQueueError) -> Self {
        match err {
            CommandQueueError::QueueFull => OrchestratorError::QueueFull,
            CommandQueueError::CommandNotFound(id) => OrchestratorError::CommandNotFound(id),
            CommandQueueError::AlreadyApproved(_) => {
                OrchestratorError::InvalidStateTransition(err.to_string())
            }
        }
    }
}

/// Result type for orchestrator operations.
pub type OrchestratorResult<T> = Result<T, OrchestratorError>;

/// Central coordinator for agent orchestration.
///
/// Implements the state machine from AgentOrchestration.tla.
pub struct Orchestrator {
    /// Configuration
    config: OrchestratorConfig,
    /// All agents by ID
    agents: HashMap<AgentId, Agent>,
    /// Command queue
    command_queue: CommandQueue,
    /// Execution manager
    executions: ExecutionManager,
    /// Terminal pool
    terminal_pool: TerminalPool,
    /// Set of completed command IDs (for dependency resolution)
    completed_commands: HashSet<CommandId>,
    /// Next agent ID
    next_agent_id: u64,
    /// Approval manager for dangerous operations
    approval_manager: ApprovalManager,
    /// Whether to require approval for all commands (strict mode)
    require_approval: bool,
    /// Domain registry for spawning panes (Phase 12)
    domain_registry: Option<Arc<DomainRegistry>>,
    /// Default domain for pane spawning (Phase 12)
    default_domain: Option<Arc<dyn Domain>>,
}

impl Orchestrator {
    /// Create a new orchestrator with the given configuration.
    pub fn new(config: OrchestratorConfig) -> Self {
        Self {
            command_queue: CommandQueue::new(config.max_queue_size),
            executions: ExecutionManager::new(config.max_executions),
            terminal_pool: TerminalPool::new(config.max_terminals),
            agents: HashMap::new(),
            completed_commands: HashSet::new(),
            next_agent_id: 0,
            config,
            approval_manager: ApprovalManager::with_defaults(),
            require_approval: false,
            domain_registry: None,
            default_domain: None,
        }
    }

    /// Create a new orchestrator with custom approval configuration.
    pub fn with_approval_config(
        config: OrchestratorConfig,
        approval_config: ApprovalConfig,
    ) -> Self {
        Self {
            command_queue: CommandQueue::new(config.max_queue_size),
            executions: ExecutionManager::new(config.max_executions),
            terminal_pool: TerminalPool::new(config.max_terminals),
            agents: HashMap::new(),
            completed_commands: HashSet::new(),
            next_agent_id: 0,
            config,
            approval_manager: ApprovalManager::new(approval_config),
            require_approval: false,
            domain_registry: None,
            default_domain: None,
        }
    }

    /// Create an orchestrator with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(OrchestratorConfig::default())
    }

    /// Enable strict approval mode (require approval for all commands).
    pub fn set_require_approval(&mut self, require: bool) {
        self.require_approval = require;
    }

    /// Check if strict approval mode is enabled.
    pub fn requires_approval(&self) -> bool {
        self.require_approval
    }

    /// Set the approval callback for UI notifications.
    pub fn set_approval_callback(&mut self, callback: Box<dyn ApprovalCallback>) {
        self.approval_manager.set_callback(callback);
    }

    // =========================================================================
    // Agent Management
    // =========================================================================

    /// Spawn a new agent with the given capabilities.
    ///
    /// Implements `SpawnAgent` from TLA+ spec.
    pub fn spawn_agent(&mut self, capabilities: &[Capability]) -> OrchestratorResult<AgentId> {
        if self.agents.len() >= self.config.max_agents {
            return Err(OrchestratorError::MaxAgentsReached);
        }
        if capabilities.is_empty() {
            return Err(OrchestratorError::InvalidStateTransition(
                "Agent must have at least one capability".to_string(),
            ));
        }

        let id = AgentId(self.next_agent_id);
        self.next_agent_id += 1;

        let agent = Agent::new(id, capabilities.iter().copied().collect());
        self.agents.insert(id, agent);
        Ok(id)
    }

    /// Get an agent by ID.
    pub fn get_agent(&self, id: AgentId) -> Option<&Agent> {
        self.agents.get(&id)
    }

    /// Get a mutable reference to an agent.
    fn get_agent_mut(&mut self, id: AgentId) -> OrchestratorResult<&mut Agent> {
        self.agents
            .get_mut(&id)
            .ok_or(OrchestratorError::AgentNotFound(id))
    }

    /// Get all idle agents.
    pub fn idle_agents(&self) -> impl Iterator<Item = &Agent> {
        self.agents.values().filter(|a| a.state == AgentState::Idle)
    }

    /// Get all agents.
    pub fn agents(&self) -> impl Iterator<Item = &Agent> {
        self.agents.values()
    }

    /// Reset an agent to Idle state.
    ///
    /// Implements `ResetAgent` from TLA+ spec.
    pub fn reset_agent(&mut self, agent_id: AgentId) -> OrchestratorResult<()> {
        let agent = self.get_agent_mut(agent_id)?;
        agent
            .reset()
            .map_err(|e| OrchestratorError::InvalidStateTransition(e.to_string()))
    }

    // =========================================================================
    // Command Management
    // =========================================================================

    /// Queue a command for execution.
    ///
    /// Implements `QueueCommand` from TLA+ spec.
    pub fn queue_command(&mut self, command: Command) -> OrchestratorResult<CommandId> {
        // Validate dependencies exist
        let valid_deps: HashSet<_> = self
            .completed_commands
            .union(&self.command_queue.command_ids())
            .copied()
            .collect();
        if !command.dependencies.is_subset(&valid_deps) {
            return Err(OrchestratorError::DependenciesNotSatisfied);
        }

        Ok(self.command_queue.enqueue(command)?)
    }

    /// Approve a command for execution.
    ///
    /// Implements `ApproveCommand` from TLA+ spec.
    pub fn approve_command(&mut self, command_id: CommandId) -> OrchestratorResult<()> {
        Ok(self.command_queue.approve(command_id)?)
    }

    /// Get a command by ID.
    pub fn get_command(&self, id: CommandId) -> Option<&Command> {
        self.command_queue.get(id)
    }

    /// Get commands ready for assignment.
    pub fn ready_commands(&self) -> Vec<CommandId> {
        let assigned: HashSet<_> = self
            .agents
            .values()
            .filter_map(|a| a.current_command_id)
            .collect();
        self.command_queue
            .ready_commands(&self.completed_commands, &assigned)
            .map(|c| c.id)
            .collect()
    }

    // =========================================================================
    // Approval Workflow
    // =========================================================================

    /// Request approval for a command before execution.
    ///
    /// This creates an approval request that must be approved before
    /// the command can be assigned and executed.
    ///
    /// Returns the approval request ID.
    pub fn request_approval(
        &mut self,
        agent_id: AgentId,
        command_id: CommandId,
    ) -> OrchestratorResult<ApprovalRequestId> {
        // Verify agent exists
        if !self.agents.contains_key(&agent_id) {
            return Err(OrchestratorError::AgentNotFound(agent_id));
        }

        // Get command info
        let command = self
            .command_queue
            .get(command_id)
            .ok_or(OrchestratorError::CommandNotFound(command_id))?;

        // Convert command type to approval action
        let action = command_type_to_action(command.command_type);
        let description = format!(
            "{}: {}",
            command.command_type,
            if command.description.is_empty() {
                &command.payload
            } else {
                &command.description
            }
        );

        self.approval_manager
            .submit_request(agent_id, action, description)
            .map_err(|e| OrchestratorError::InvalidStateTransition(e.to_string()))
    }

    /// Approve a pending approval request.
    ///
    /// After approval, the associated command can be assigned and executed.
    pub fn approve_request(&mut self, request_id: ApprovalRequestId) -> ApprovalResult<()> {
        self.approval_manager.approve(request_id)
    }

    /// Reject a pending approval request.
    pub fn reject_request(&mut self, request_id: ApprovalRequestId) -> ApprovalResult<()> {
        self.approval_manager.reject(request_id)
    }

    /// Cancel a pending approval request.
    ///
    /// Only the agent that submitted the request can cancel it.
    pub fn cancel_request(
        &mut self,
        agent_id: AgentId,
        request_id: ApprovalRequestId,
    ) -> ApprovalResult<()> {
        self.approval_manager.cancel(agent_id, request_id)
    }

    /// Process approval timeouts.
    ///
    /// Returns the number of requests that timed out.
    pub fn process_approval_timeouts(&mut self) -> usize {
        self.approval_manager.process_timeouts()
    }

    /// Check if an approval request is approved.
    pub fn is_request_approved(&self, request_id: ApprovalRequestId) -> bool {
        self.approval_manager.is_approved(request_id)
    }

    /// Check if an approval request is pending.
    pub fn is_request_pending(&self, request_id: ApprovalRequestId) -> bool {
        self.approval_manager.is_pending(request_id)
    }

    /// Get the number of pending approval requests.
    pub fn pending_approval_count(&self) -> usize {
        self.approval_manager.pending_count()
    }

    /// Get pending approval requests for a specific agent.
    pub fn pending_approvals_for_agent(&self, agent_id: AgentId) -> usize {
        self.approval_manager.pending_count_for_agent(agent_id)
    }

    /// Get the approval manager for advanced queries.
    pub fn approval_manager(&self) -> &ApprovalManager {
        &self.approval_manager
    }

    /// Get mutable access to the approval manager.
    pub fn approval_manager_mut(&mut self) -> &mut ApprovalManager {
        &mut self.approval_manager
    }

    // =========================================================================
    // Domain Management (Phase 12)
    // =========================================================================

    /// Set the domain registry for spawning panes.
    ///
    /// When a domain registry is set, the orchestrator can spawn real panes
    /// for command execution. Without a registry, terminal slots remain
    /// as abstract placeholders.
    pub fn set_domain_registry(&mut self, registry: Arc<DomainRegistry>) {
        self.domain_registry = Some(registry);
    }

    /// Get the domain registry (if set).
    pub fn domain_registry(&self) -> Option<&Arc<DomainRegistry>> {
        self.domain_registry.as_ref()
    }

    /// Set a default domain for pane spawning.
    ///
    /// This domain is used when no specific domain is specified for a command.
    /// Setting this also enables domain-based execution.
    pub fn set_default_domain(&mut self, domain: Arc<dyn Domain>) {
        self.default_domain = Some(domain);
    }

    /// Get the default domain (if set).
    pub fn default_domain(&self) -> Option<&Arc<dyn Domain>> {
        self.default_domain.as_ref()
    }

    /// Check if domain-based execution is enabled.
    ///
    /// Returns true if either a default domain or domain registry is configured.
    pub fn has_domain_support(&self) -> bool {
        self.default_domain.is_some() || self.domain_registry.is_some()
    }

    /// Get the domain to use for spawning panes.
    ///
    /// Returns the default domain, or the first domain from the registry.
    fn get_spawn_domain(&self) -> Option<Arc<dyn Domain>> {
        if let Some(ref domain) = self.default_domain {
            return Some(Arc::clone(domain));
        }
        if let Some(ref registry) = self.domain_registry {
            return registry.default_domain();
        }
        None
    }

    // =========================================================================
    // Assignment and Execution
    // =========================================================================

    /// Assign a command to an agent.
    ///
    /// Implements `AssignCommand` from TLA+ spec.
    ///
    /// Enforces:
    /// - INV-ORCH-1: No double assignment
    /// - INV-ORCH-5: Capability matching
    pub fn assign_command(
        &mut self,
        agent_id: AgentId,
        command_id: CommandId,
    ) -> OrchestratorResult<()> {
        // Get command to check capabilities
        let command = self
            .command_queue
            .get(command_id)
            .ok_or(OrchestratorError::CommandNotFound(command_id))?;

        if !command.approved {
            return Err(OrchestratorError::NotApproved);
        }

        // Check dependencies
        if !command.dependencies.is_subset(&self.completed_commands) {
            return Err(OrchestratorError::DependenciesNotSatisfied);
        }

        // Check already assigned
        for agent in self.agents.values() {
            if agent.current_command_id == Some(command_id) {
                return Err(OrchestratorError::InvalidStateTransition(format!(
                    "Command {} already assigned to agent {}",
                    command_id, agent.id
                )));
            }
        }

        let required_caps = command.required_capabilities.clone();

        // Check agent capabilities
        let agent = self.get_agent_mut(agent_id)?;
        if !agent.has_capabilities(&required_caps) {
            return Err(OrchestratorError::CapabilityMismatch);
        }

        agent
            .assign(command_id)
            .map_err(|e| OrchestratorError::InvalidStateTransition(e.to_string()))
    }

    /// Begin execution of assigned command.
    ///
    /// Implements `BeginExecution` from TLA+ spec.
    ///
    /// Enforces:
    /// - INV-ORCH-3: Terminal exclusivity
    /// - INV-ORCH-6: Executing agent has terminal
    /// - INV-ORCH-7: Only approved commands
    ///
    /// ## Phase 12: Domain Integration
    ///
    /// If a domain is configured (via `set_default_domain` or `set_domain_registry`),
    /// this method will spawn a real pane and terminal for the execution:
    /// 1. Spawn pane from domain (80x24 default size)
    /// 2. Create terminal state machine
    /// 3. Attach pane + terminal to the allocated slot
    ///
    /// If no domain is configured, the slot remains a placeholder (legacy behavior).
    pub fn begin_execution(&mut self, agent_id: AgentId) -> OrchestratorResult<ExecutionId> {
        // Get agent state
        let agent = self.get_agent_mut(agent_id)?;
        let command_id =
            agent
                .current_command_id
                .ok_or(OrchestratorError::InvalidStateTransition(
                    "No command assigned".to_string(),
                ))?;

        // Verify command is approved (INV-ORCH-7)
        let command = self
            .command_queue
            .get(command_id)
            .ok_or(OrchestratorError::CommandNotFound(command_id))?;
        if !command.approved {
            return Err(OrchestratorError::NotApproved);
        }

        if !self.executions.can_start() {
            return Err(OrchestratorError::MaxExecutionsReached);
        }

        // Allocate terminal (INV-ORCH-3)
        let terminal_id = self
            .terminal_pool
            .allocate(ExecutionId(0)) // Temporary, will update
            .map_err(|_| OrchestratorError::NoTerminalsAvailable)?;

        // Phase 12: Spawn pane if domain is configured
        if let Some(domain) = self.get_spawn_domain() {
            // Default terminal size (could be made configurable)
            let cols = 80;
            let rows = 24;

            // Spawn pane from domain
            let pane = match domain.spawn_pane(cols, rows, crate::domain::SpawnConfig::default()) {
                Ok(pane) => pane,
                Err(e) => {
                    // Release terminal slot on spawn failure
                    let _ = self.terminal_pool.release(terminal_id);
                    return Err(OrchestratorError::SpawnFailed(e.to_string()));
                }
            };

            // Create terminal state machine
            let terminal = crate::terminal::Terminal::new(cols, rows);

            // Attach resources to slot
            if let Some(slot) = self.terminal_pool.get_mut(terminal_id) {
                slot.attach_pane(pane, domain.domain_id());
                slot.attach_terminal(terminal);
            }
        }

        // Start execution
        let exec_id = if let Ok(exec_id) = self.executions.start(agent_id, command_id, terminal_id)
        {
            exec_id
        } else {
            let _ = self.terminal_pool.release(terminal_id);
            return Err(OrchestratorError::MaxExecutionsReached);
        };

        // Update terminal with real execution ID
        if let Some(slot) = self.terminal_pool.get_mut(terminal_id) {
            slot.current_execution_id = Some(exec_id);
        }

        // Transition agent state
        let agent = self.get_agent_mut(agent_id)?;
        agent
            .begin_execution(exec_id)
            .map_err(|e| OrchestratorError::InvalidStateTransition(e.to_string()))?;

        Ok(exec_id)
    }

    /// Complete execution successfully.
    ///
    /// Implements `CompleteExecution` from TLA+ spec.
    pub fn complete_execution(
        &mut self,
        agent_id: AgentId,
        exit_code: i32,
    ) -> OrchestratorResult<()> {
        // Get execution info
        let agent = self
            .agents
            .get(&agent_id)
            .ok_or(OrchestratorError::AgentNotFound(agent_id))?;
        let exec_id =
            agent
                .current_execution_id
                .ok_or(OrchestratorError::InvalidStateTransition(
                    "Not executing".to_string(),
                ))?;
        let command_id =
            agent
                .current_command_id
                .ok_or(OrchestratorError::InvalidStateTransition(
                    "No command".to_string(),
                ))?;

        // Get terminal ID
        let terminal_id = self
            .executions
            .get(exec_id)
            .ok_or(OrchestratorError::ExecutionNotFound(exec_id))?
            .terminal_id;

        // Complete execution
        self.executions
            .get_mut(exec_id)
            .ok_or(OrchestratorError::ExecutionNotFound(exec_id))?
            .succeed(exit_code)
            .map_err(|e| OrchestratorError::InvalidStateTransition(e.to_string()))?;

        // Release terminal
        self.terminal_pool
            .release(terminal_id)
            .map_err(|e| OrchestratorError::InvalidStateTransition(e.to_string()))?;

        // Complete agent
        let agent = self.get_agent_mut(agent_id)?;
        agent
            .complete()
            .map_err(|e| OrchestratorError::InvalidStateTransition(e.to_string()))?;

        // Add to completed commands for dependency resolution
        self.completed_commands.insert(command_id);

        // Remove from queue
        self.command_queue.remove(command_id);

        Ok(())
    }

    /// Fail execution.
    ///
    /// Implements `FailExecution` from TLA+ spec.
    pub fn fail_execution(
        &mut self,
        agent_id: AgentId,
        error: impl Into<String>,
    ) -> OrchestratorResult<()> {
        let error_msg = error.into();

        // Get execution info
        let agent = self
            .agents
            .get(&agent_id)
            .ok_or(OrchestratorError::AgentNotFound(agent_id))?;
        let exec_id =
            agent
                .current_execution_id
                .ok_or(OrchestratorError::InvalidStateTransition(
                    "Not executing".to_string(),
                ))?;

        // Get terminal ID
        let terminal_id = self
            .executions
            .get(exec_id)
            .ok_or(OrchestratorError::ExecutionNotFound(exec_id))?
            .terminal_id;

        // Fail execution
        self.executions
            .get_mut(exec_id)
            .ok_or(OrchestratorError::ExecutionNotFound(exec_id))?
            .fail(&error_msg)
            .map_err(|e| OrchestratorError::InvalidStateTransition(e.to_string()))?;

        // Release terminal
        self.terminal_pool
            .release(terminal_id)
            .map_err(|e| OrchestratorError::InvalidStateTransition(e.to_string()))?;

        // Fail agent
        let agent = self.get_agent_mut(agent_id)?;
        agent
            .fail()
            .map_err(|e| OrchestratorError::InvalidStateTransition(e.to_string()))?;

        Ok(())
    }

    /// Fail execution with a specific exit code.
    ///
    /// Like `fail_execution` but also stores the exit code on the execution record.
    /// Used by completion detection when a process exits with a non-zero code.
    pub fn fail_execution_with_exit_code(
        &mut self,
        agent_id: AgentId,
        exit_code: i32,
        error: impl Into<String>,
    ) -> OrchestratorResult<()> {
        let error_msg = error.into();

        // Get execution info
        let agent = self
            .agents
            .get(&agent_id)
            .ok_or(OrchestratorError::AgentNotFound(agent_id))?;
        let exec_id =
            agent
                .current_execution_id
                .ok_or(OrchestratorError::InvalidStateTransition(
                    "Not executing".to_string(),
                ))?;

        // Get terminal ID
        let terminal_id = self
            .executions
            .get(exec_id)
            .ok_or(OrchestratorError::ExecutionNotFound(exec_id))?
            .terminal_id;

        // Fail execution with exit code
        self.executions
            .get_mut(exec_id)
            .ok_or(OrchestratorError::ExecutionNotFound(exec_id))?
            .fail_with_exit_code(exit_code, &error_msg)
            .map_err(|e| OrchestratorError::InvalidStateTransition(e.to_string()))?;

        // Release terminal
        self.terminal_pool
            .release(terminal_id)
            .map_err(|e| OrchestratorError::InvalidStateTransition(e.to_string()))?;

        // Fail agent
        let agent = self.get_agent_mut(agent_id)?;
        agent
            .fail()
            .map_err(|e| OrchestratorError::InvalidStateTransition(e.to_string()))?;

        Ok(())
    }

    /// Cancel execution.
    ///
    /// Implements `CancelExecution` from TLA+ spec.
    pub fn cancel_execution(&mut self, agent_id: AgentId) -> OrchestratorResult<()> {
        let agent = self
            .agents
            .get(&agent_id)
            .ok_or(OrchestratorError::AgentNotFound(agent_id))?;

        // Release resources if executing
        if let Some(exec_id) = agent.current_execution_id {
            if let Some(exec) = self.executions.get(exec_id) {
                let terminal_id = exec.terminal_id;

                self.executions
                    .get_mut(exec_id)
                    .ok_or(OrchestratorError::ExecutionNotFound(exec_id))?
                    .cancel()
                    .map_err(|e| OrchestratorError::InvalidStateTransition(e.to_string()))?;

                self.terminal_pool
                    .release(terminal_id)
                    .map_err(|e| OrchestratorError::InvalidStateTransition(e.to_string()))?;
            }
        }

        // Cancel agent
        let agent = self.get_agent_mut(agent_id)?;
        agent
            .cancel()
            .map_err(|e| OrchestratorError::InvalidStateTransition(e.to_string()))?;

        Ok(())
    }

    // =========================================================================
    // Automatic Scheduling
    // =========================================================================

    /// Automatically assign ready commands to idle, capable agents.
    ///
    /// Returns the number of assignments made.
    pub fn auto_assign(&mut self) -> usize {
        let ready = self.ready_commands();
        let mut assignments = 0;

        for cmd_id in ready {
            // Get command capabilities
            let required_caps = match self.command_queue.get(cmd_id) {
                Some(cmd) => cmd.required_capabilities.clone(),
                None => continue,
            };

            // Find capable idle agent
            let agent_id = self
                .agents
                .values()
                .find(|a| a.state == AgentState::Idle && a.has_capabilities(&required_caps))
                .map(|a| a.id);

            if let Some(agent_id) = agent_id {
                if self.assign_command(agent_id, cmd_id).is_ok() {
                    assignments += 1;
                }
            }
        }

        assignments
    }

    /// Automatically start executions for assigned agents.
    ///
    /// Returns the number of executions started.
    pub fn auto_execute(&mut self) -> usize {
        let assigned: Vec<_> = self
            .agents
            .values()
            .filter(|a| a.state == AgentState::Assigned)
            .map(|a| a.id)
            .collect();

        let mut started = 0;
        for agent_id in assigned {
            if self.begin_execution(agent_id).is_ok() {
                started += 1;
            }
        }

        started
    }

    /// Run one step of the orchestrator.
    ///
    /// 1. Auto-assign ready commands
    /// 2. Auto-start executions
    ///
    /// Returns (assignments, executions_started).
    pub fn step(&mut self) -> (usize, usize) {
        let assignments = self.auto_assign();
        let executions = self.auto_execute();
        (assignments, executions)
    }

    // =========================================================================
    // Query Methods
    // =========================================================================

    /// Get execution by ID.
    pub fn get_execution(&self, id: ExecutionId) -> Option<&Execution> {
        self.executions.get(id)
    }

    /// Get all running executions.
    pub fn running_executions(&self) -> impl Iterator<Item = &Execution> {
        self.executions.running()
    }

    /// Get the set of completed commands.
    pub fn completed_commands(&self) -> &HashSet<CommandId> {
        &self.completed_commands
    }

    /// Get terminal pool statistics.
    pub fn terminal_stats(&self) -> (usize, usize) {
        (
            self.terminal_pool.available_count(),
            self.terminal_pool.in_use_count(),
        )
    }

    /// Get number of active (running) executions.
    pub fn active_execution_count(&self) -> usize {
        self.executions.active_count()
    }

    /// Check if any terminals are available.
    pub fn has_available_terminals(&self) -> bool {
        self.terminal_pool.has_available()
    }

    /// Find execution for an agent (if running).
    pub fn find_execution_by_agent(&self, agent_id: AgentId) -> Option<&Execution> {
        self.executions.find_by_agent(agent_id)
    }

    /// Cleanup old completed executions.
    pub fn cleanup(&mut self, max_age: Duration) {
        self.executions.cleanup(max_age);
    }

    // =========================================================================
    // Completion Detection (Phase 12 Step 4)
    // =========================================================================

    /// Check a single execution for completion.
    ///
    /// If the pane's process has exited, transitions the execution to
    /// Succeeded or Failed based on exit status, releases resources,
    /// and updates the agent state.
    ///
    /// Returns `Some((exit_code, success))` if the execution completed,
    /// `None` if still running or no resources are attached.
    pub fn check_execution_completion(
        &mut self,
        exec_id: ExecutionId,
    ) -> OrchestratorResult<Option<(i32, bool)>> {
        // Get execution info
        let exec = self
            .executions
            .get(exec_id)
            .ok_or(OrchestratorError::ExecutionNotFound(exec_id))?;

        // Only check running executions
        if !exec.state.is_running() {
            return Ok(None);
        }

        let agent_id = exec.agent_id;
        let terminal_id = exec.terminal_id;

        // Get the slot and check if pane has completed
        let slot = self.terminal_pool.get(terminal_id).ok_or(
            OrchestratorError::InvalidStateTransition(format!(
                "Terminal slot {} not found for execution {}",
                terminal_id, exec_id
            )),
        )?;

        // If no pane is attached, we can't detect completion this way
        let pane = match slot.pane() {
            Some(p) => p,
            None => return Ok(None),
        };

        // Check if pane is still alive
        if pane.is_alive() {
            return Ok(None);
        }

        // Pane has exited - get exit status
        let exit_code = pane.exit_status().unwrap_or(-1);
        let success = exit_code == 0;

        // Transition to completed state
        if success {
            self.complete_execution(agent_id, exit_code)?;
        } else {
            self.fail_execution_with_exit_code(
                agent_id,
                exit_code,
                format!("Process exited with code {}", exit_code),
            )?;
        }

        Ok(Some((exit_code, success)))
    }

    /// Check all running executions for completion.
    ///
    /// Returns the number of executions that completed.
    pub fn check_all_completions(&mut self) -> usize {
        // Collect running execution IDs first to avoid borrow issues
        let running_ids: Vec<ExecutionId> = self.executions.running().map(|e| e.id).collect();

        let mut completed = 0;
        for exec_id in running_ids {
            match self.check_execution_completion(exec_id) {
                Ok(Some(_)) => completed += 1,
                Ok(None) => {}
                Err(_) => {}
            }
        }
        completed
    }

    /// Poll for pane output and completion.
    ///
    /// This method:
    /// 1. Reads available output from each pane
    /// 2. Feeds output into the terminal parser
    /// 3. Appends raw output to the execution buffers
    /// 4. Checks for completion
    ///
    /// Returns the number of executions that completed.
    ///
    /// This is the main polling method for runtime loops.
    pub fn poll_executions(&mut self) -> usize {
        // Collect running execution IDs and terminal IDs
        let running: Vec<(ExecutionId, TerminalSlotId)> = self
            .executions
            .running()
            .map(|e| (e.id, e.terminal_id))
            .collect();

        // Process each execution
        for (exec_id, terminal_id) in &running {
            // Read from pane and feed to terminal
            if let Some(slot) = self.terminal_pool.get_mut(*terminal_id) {
                if let Some(pane) = slot.pane() {
                    let mut buf = [0u8; 4096];
                    if let Ok(n) = pane.read(&mut buf) {
                        if n > 0 {
                            // Feed to terminal parser
                            if let Some(terminal) = slot.terminal_mut() {
                                terminal.process(&buf[..n]);
                            }
                            // Append to execution stdout buffer
                            if let Some(exec) = self.executions.get_mut(*exec_id) {
                                exec.append_stdout(&buf[..n]);
                            }
                        }
                    }
                }
            }
        }

        // Check for completions
        self.check_all_completions()
    }

    // =========================================================================
    // Invariant Verification (for testing)
    // =========================================================================

    /// Verify INV-ORCH-1: No command assigned to multiple agents.
    #[cfg(test)]
    fn verify_no_double_assignment(&self) -> bool {
        let mut assigned_commands = HashSet::new();
        for agent in self.agents.values() {
            if let Some(cmd_id) = agent.current_command_id {
                if !assigned_commands.insert(cmd_id) {
                    return false;
                }
            }
        }
        true
    }

    /// Verify INV-ORCH-2: Every running execution has an executing agent.
    #[cfg(test)]
    fn verify_no_orphaned_executions(&self) -> bool {
        for exec in self.executions.running() {
            let agent = match self.agents.get(&exec.agent_id) {
                Some(a) => a,
                None => return false,
            };
            if agent.state != AgentState::Executing {
                return false;
            }
            if agent.current_execution_id != Some(exec.id) {
                return false;
            }
        }
        true
    }

    /// Verify INV-ORCH-3: Terminal exclusivity.
    #[cfg(test)]
    fn verify_terminal_exclusivity(&self) -> bool {
        for slot in self.terminal_pool.in_use() {
            let running_count = self
                .executions
                .running()
                .filter(|e| e.terminal_id == slot.id)
                .count();
            if running_count != 1 {
                return false;
            }
        }
        true
    }

    /// Verify all safety invariants.
    #[cfg(test)]
    pub fn verify_invariants(&self) -> bool {
        self.verify_no_double_assignment()
            && self.verify_no_orphaned_executions()
            && self.verify_terminal_exclusivity()
    }
}

/// A cloneable snapshot of orchestrator state.
///
/// This provides a point-in-time copy of the orchestrator's state that can
/// be cloned for testing, debugging, or inspection. Unlike the full `Orchestrator`,
/// this excludes non-cloneable components (callbacks, trait objects).
#[derive(Debug, Clone)]
pub struct OrchestratorSnapshot {
    /// Configuration
    pub config: OrchestratorConfig,
    /// All agents by ID
    pub agents: HashMap<AgentId, Agent>,
    /// Completed command IDs
    pub completed_commands: HashSet<CommandId>,
    /// Number of pending commands in queue
    pub queue_size: usize,
    /// Number of active executions
    pub active_executions: usize,
    /// Number of available terminals
    pub available_terminals: usize,
    /// Number of terminals in use
    pub terminals_in_use: usize,
    /// Whether strict approval mode is enabled
    pub require_approval: bool,
    /// Number of pending approval requests
    pub pending_approvals: usize,
    /// Whether domain support is configured
    pub has_domain_support: bool,
}

impl Orchestrator {
    /// Create a cloneable snapshot of the current orchestrator state.
    ///
    /// This is useful for:
    /// - Testing and assertions
    /// - Debugging and inspection
    /// - Serialization (the snapshot can derive Serialize)
    /// - Comparing state before/after operations
    ///
    /// # Example
    ///
    /// ```
    /// use dterm_core::agent::{Orchestrator, OrchestratorConfig, Capability};
    ///
    /// let mut orch = Orchestrator::new(OrchestratorConfig::default());
    /// let agent_id = orch.spawn_agent(&[Capability::Shell]).unwrap();
    ///
    /// let snapshot = orch.snapshot();
    /// assert_eq!(snapshot.agents.len(), 1);
    /// assert!(snapshot.agents.contains_key(&agent_id));
    ///
    /// // Snapshot is cloneable
    /// let snapshot2 = snapshot.clone();
    /// assert_eq!(snapshot.agents.len(), snapshot2.agents.len());
    /// ```
    #[must_use]
    pub fn snapshot(&self) -> OrchestratorSnapshot {
        OrchestratorSnapshot {
            config: self.config.clone(),
            agents: self.agents.clone(),
            completed_commands: self.completed_commands.clone(),
            queue_size: self.command_queue.len(),
            active_executions: self.executions.active_count(),
            available_terminals: self.terminal_pool.available_count(),
            terminals_in_use: self.terminal_pool.in_use_count(),
            require_approval: self.require_approval,
            pending_approvals: self.approval_manager.pending_count(),
            has_domain_support: self.has_domain_support(),
        }
    }
}

impl fmt::Debug for Orchestrator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Orchestrator")
            .field("agents", &self.agents.len())
            .field("queue_size", &self.command_queue.len())
            .field("active_executions", &self.executions.active_count())
            .field("terminals_available", &self.terminal_pool.available_count())
            .field("completed_commands", &self.completed_commands.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::CommandType;

    fn create_orchestrator() -> Orchestrator {
        Orchestrator::new(OrchestratorConfig {
            max_agents: 5,
            max_terminals: 3,
            max_queue_size: 10,
            max_executions: 3,
        })
    }

    #[test]
    fn test_spawn_agent() {
        let mut orch = create_orchestrator();

        let id = orch.spawn_agent(&[Capability::Shell]).unwrap();
        let agent = orch.get_agent(id).unwrap();

        assert_eq!(agent.state, AgentState::Idle);
        assert!(agent.capabilities.contains(&Capability::Shell));
    }

    #[test]
    fn test_spawn_agent_max() {
        let mut orch = create_orchestrator();

        for _ in 0..5 {
            orch.spawn_agent(&[Capability::Shell]).unwrap();
        }

        // Sixth should fail
        assert!(matches!(
            orch.spawn_agent(&[Capability::Shell]),
            Err(OrchestratorError::MaxAgentsReached)
        ));
    }

    #[test]
    fn test_queue_command() {
        let mut orch = create_orchestrator();

        let cmd = Command::shell(CommandId(0), "echo hello");
        let id = orch.queue_command(cmd).unwrap();

        assert!(orch.get_command(id).is_some());
    }

    #[test]
    fn test_full_lifecycle() {
        let mut orch = create_orchestrator();

        // Spawn agent
        let agent_id = orch.spawn_agent(&[Capability::Shell]).unwrap();

        // Queue approved command
        let cmd = Command::shell(CommandId(0), "echo hello");
        let cmd_id = orch.queue_command(cmd).unwrap();

        // Assign
        orch.assign_command(agent_id, cmd_id).unwrap();
        assert_eq!(
            orch.get_agent(agent_id).unwrap().state,
            AgentState::Assigned
        );

        // Execute
        let _exec_id = orch.begin_execution(agent_id).unwrap();
        assert_eq!(
            orch.get_agent(agent_id).unwrap().state,
            AgentState::Executing
        );
        assert!(orch.verify_invariants());

        // Complete
        orch.complete_execution(agent_id, 0).unwrap();
        assert_eq!(
            orch.get_agent(agent_id).unwrap().state,
            AgentState::Completed
        );
        assert!(orch.completed_commands.contains(&cmd_id));
        assert!(orch.verify_invariants());

        // Reset
        orch.reset_agent(agent_id).unwrap();
        assert_eq!(orch.get_agent(agent_id).unwrap().state, AgentState::Idle);
    }

    #[test]
    fn test_capability_mismatch() {
        let mut orch = create_orchestrator();

        // Agent with Shell capability
        let agent_id = orch.spawn_agent(&[Capability::Shell]).unwrap();

        // Command requiring Net capability
        let cmd = Command::builder(CommandType::Network)
            .approved()
            .build(CommandId(0));
        let cmd_id = orch.queue_command(cmd).unwrap();

        // Assignment should fail
        assert!(matches!(
            orch.assign_command(agent_id, cmd_id),
            Err(OrchestratorError::CapabilityMismatch)
        ));
    }

    #[test]
    fn test_unapproved_command() {
        let mut orch = create_orchestrator();

        let agent_id = orch.spawn_agent(&[Capability::Shell]).unwrap();

        // Unapproved command
        let cmd = Command::builder(CommandType::Shell).build(CommandId(0));
        let cmd_id = orch.queue_command(cmd).unwrap();

        // Assignment should fail
        assert!(matches!(
            orch.assign_command(agent_id, cmd_id),
            Err(OrchestratorError::NotApproved)
        ));

        // Approve and retry
        orch.approve_command(cmd_id).unwrap();
        assert!(orch.assign_command(agent_id, cmd_id).is_ok());
    }

    #[test]
    fn test_dependencies() {
        let mut orch = create_orchestrator();

        // First command
        let cmd1 = Command::shell(CommandId(0), "cmd1");
        let cmd1_id = orch.queue_command(cmd1).unwrap();

        // Second command depending on first
        let cmd2 = Command::builder(CommandType::Shell)
            .approved()
            .depends_on(cmd1_id)
            .build(CommandId(0));
        let cmd2_id = orch.queue_command(cmd2).unwrap();

        let agent_id = orch.spawn_agent(&[Capability::Shell]).unwrap();

        // Cannot assign cmd2 before cmd1 completes
        assert!(matches!(
            orch.assign_command(agent_id, cmd2_id),
            Err(OrchestratorError::DependenciesNotSatisfied)
        ));

        // Complete cmd1
        orch.assign_command(agent_id, cmd1_id).unwrap();
        orch.begin_execution(agent_id).unwrap();
        orch.complete_execution(agent_id, 0).unwrap();
        orch.reset_agent(agent_id).unwrap();

        // Now can assign cmd2
        assert!(orch.assign_command(agent_id, cmd2_id).is_ok());
    }

    #[test]
    fn test_auto_assign() {
        let mut orch = create_orchestrator();

        // Spawn 2 agents
        orch.spawn_agent(&[Capability::Shell]).unwrap();
        orch.spawn_agent(&[Capability::Shell]).unwrap();

        // Queue 3 commands
        for _ in 0..3 {
            orch.queue_command(Command::shell(CommandId(0), "cmd"))
                .unwrap();
        }

        // Auto-assign should assign 2 (one per idle agent)
        let assigned = orch.auto_assign();
        assert_eq!(assigned, 2);
        assert!(orch.verify_invariants());
    }

    #[test]
    fn test_step() {
        let mut orch = create_orchestrator();

        orch.spawn_agent(&[Capability::Shell]).unwrap();
        orch.queue_command(Command::shell(CommandId(0), "cmd"))
            .unwrap();

        let (assigned, started) = orch.step();
        assert_eq!(assigned, 1);
        assert_eq!(started, 1);
        assert!(orch.verify_invariants());
    }

    #[test]
    fn test_terminal_exhaustion() {
        let mut orch = create_orchestrator();

        // Spawn 5 agents
        for _ in 0..5 {
            orch.spawn_agent(&[Capability::Shell]).unwrap();
        }

        // Queue 5 commands
        for _ in 0..5 {
            orch.queue_command(Command::shell(CommandId(0), "cmd"))
                .unwrap();
        }

        // Run multiple steps - should only start 3 (terminal limit)
        for _ in 0..3 {
            orch.step();
        }

        // Only 3 terminals available
        assert_eq!(orch.executions.active_count(), 3);
        assert!(orch.verify_invariants());
    }

    #[test]
    fn test_max_executions_releases_terminal() {
        let mut orch = Orchestrator::new(OrchestratorConfig {
            max_agents: 2,
            max_terminals: 2,
            max_queue_size: 4,
            max_executions: 1,
        });

        let agent1 = orch.spawn_agent(&[Capability::Shell]).unwrap();
        let agent2 = orch.spawn_agent(&[Capability::Shell]).unwrap();

        let cmd1_id = orch
            .queue_command(Command::shell(CommandId(0), "cmd1"))
            .unwrap();
        let cmd2_id = orch
            .queue_command(Command::shell(CommandId(0), "cmd2"))
            .unwrap();

        orch.assign_command(agent1, cmd1_id).unwrap();
        orch.assign_command(agent2, cmd2_id).unwrap();

        orch.begin_execution(agent1).unwrap();

        let err = orch.begin_execution(agent2).unwrap_err();
        assert!(matches!(err, OrchestratorError::MaxExecutionsReached));

        let (available, in_use) = orch.terminal_stats();
        assert_eq!(available, 1);
        assert_eq!(in_use, 1);
    }

    // =========================================================================
    // Approval Workflow Tests
    // =========================================================================

    #[test]
    fn test_approval_workflow_basic() {
        let mut orch = create_orchestrator();

        // Spawn agent
        let agent_id = orch.spawn_agent(&[Capability::Shell]).unwrap();

        // Queue command (unapproved)
        let cmd = Command::builder(CommandType::Shell)
            .payload("echo dangerous")
            .build(CommandId(0));
        let cmd_id = orch.queue_command(cmd).unwrap();

        // Request approval
        let req_id = orch.request_approval(agent_id, cmd_id).unwrap();
        assert!(orch.is_request_pending(req_id));
        assert_eq!(orch.pending_approval_count(), 1);

        // Approve
        orch.approve_request(req_id).unwrap();
        assert!(orch.is_request_approved(req_id));
        assert!(!orch.is_request_pending(req_id));
        assert_eq!(orch.pending_approval_count(), 0);
    }

    #[test]
    fn test_approval_workflow_rejection() {
        let mut orch = create_orchestrator();

        let agent_id = orch.spawn_agent(&[Capability::Shell]).unwrap();

        let cmd = Command::builder(CommandType::Shell)
            .payload("rm -rf /")
            .build(CommandId(0));
        let cmd_id = orch.queue_command(cmd).unwrap();

        let req_id = orch.request_approval(agent_id, cmd_id).unwrap();

        // Reject
        orch.reject_request(req_id).unwrap();

        // Verify rejected
        assert!(!orch.is_request_approved(req_id));
        assert!(!orch.is_request_pending(req_id));

        // Check audit log
        let audit_entries: Vec<_> = orch.approval_manager().audit_log().collect();
        assert_eq!(audit_entries.len(), 1);
        assert_eq!(
            audit_entries[0].decision,
            super::super::approval::ApprovalState::Rejected
        );
    }

    #[test]
    fn test_approval_workflow_cancellation() {
        let mut orch = create_orchestrator();

        let agent_id = orch.spawn_agent(&[Capability::Shell]).unwrap();
        let other_agent = orch.spawn_agent(&[Capability::Shell]).unwrap();

        let cmd = Command::builder(CommandType::Shell)
            .payload("echo hello")
            .build(CommandId(0));
        let cmd_id = orch.queue_command(cmd).unwrap();

        let req_id = orch.request_approval(agent_id, cmd_id).unwrap();

        // Other agent cannot cancel
        assert!(orch.cancel_request(other_agent, req_id).is_err());

        // Owner can cancel
        orch.cancel_request(agent_id, req_id).unwrap();
        assert!(!orch.is_request_pending(req_id));
    }

    #[test]
    fn test_approval_per_agent_tracking() {
        let mut orch = create_orchestrator();

        let agent1 = orch.spawn_agent(&[Capability::Shell]).unwrap();
        let agent2 = orch.spawn_agent(&[Capability::Shell]).unwrap();

        // Queue multiple commands
        let cmd1_id = orch
            .queue_command(Command::builder(CommandType::Shell).build(CommandId(0)))
            .unwrap();
        let cmd2_id = orch
            .queue_command(Command::builder(CommandType::Shell).build(CommandId(0)))
            .unwrap();
        let cmd3_id = orch
            .queue_command(Command::builder(CommandType::Shell).build(CommandId(0)))
            .unwrap();

        // Agent1 requests 2, Agent2 requests 1
        orch.request_approval(agent1, cmd1_id).unwrap();
        orch.request_approval(agent1, cmd2_id).unwrap();
        orch.request_approval(agent2, cmd3_id).unwrap();

        assert_eq!(orch.pending_approvals_for_agent(agent1), 2);
        assert_eq!(orch.pending_approvals_for_agent(agent2), 1);
        assert_eq!(orch.pending_approval_count(), 3);
    }

    #[test]
    fn test_approval_command_type_mapping() {
        let mut orch = create_orchestrator();

        let agent_id = orch.spawn_agent(&[Capability::Admin]).unwrap();

        // Admin command should map to Admin action
        let cmd = Command::builder(CommandType::Admin)
            .payload("sudo reboot")
            .build(CommandId(0));
        let cmd_id = orch.queue_command(cmd).unwrap();

        orch.request_approval(agent_id, cmd_id).unwrap();

        // Check that the action is Admin
        let pending: Vec<_> = orch.approval_manager().pending_requests().collect();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].action, super::super::approval::Action::Admin);
    }

    #[test]
    fn test_approval_manager_access() {
        let mut orch = create_orchestrator();

        // Get approval manager
        let _manager = orch.approval_manager();
        let _manager_mut = orch.approval_manager_mut();

        // Test with custom config
        let custom_config = super::super::approval::ApprovalConfig {
            max_requests: 50,
            max_per_agent: 5,
            timeout: std::time::Duration::from_secs(60),
            max_audit_entries: 100,
        };
        let orch2 =
            Orchestrator::with_approval_config(OrchestratorConfig::default(), custom_config);
        assert_eq!(orch2.approval_manager().pending_count(), 0);
    }

    #[test]
    fn test_strict_approval_mode() {
        let mut orch = create_orchestrator();

        assert!(!orch.requires_approval());

        orch.set_require_approval(true);
        assert!(orch.requires_approval());

        orch.set_require_approval(false);
        assert!(!orch.requires_approval());
    }

    #[test]
    fn test_orchestrator_config_builder() {
        // Test builder pattern for OrchestratorConfig
        let config = OrchestratorConfig::new()
            .with_max_agents(20)
            .with_max_terminals(10)
            .with_max_queue_size(200)
            .with_max_executions(8);

        assert_eq!(config.max_agents, 20);
        assert_eq!(config.max_terminals, 10);
        assert_eq!(config.max_queue_size, 200);
        assert_eq!(config.max_executions, 8);

        // Test defaults
        let default_config = OrchestratorConfig::new();
        assert_eq!(default_config.max_agents, 10);
        assert_eq!(default_config.max_terminals, 5);
        assert_eq!(default_config.max_queue_size, 100);
        assert_eq!(default_config.max_executions, 5);

        // Test partial builder
        let partial = OrchestratorConfig::new().with_max_agents(15);
        assert_eq!(partial.max_agents, 15);
        assert_eq!(partial.max_terminals, 5); // default
    }

    // =========================================================================
    // Phase 12: Domain Integration Tests
    // =========================================================================

    mod domain_tests {
        use super::*;
        use crate::domain::{
            Domain, DomainError, DomainId, DomainResult, DomainState, DomainType, Pane, PaneId,
            SpawnConfig,
        };
        use std::sync::{Arc, Mutex};

        /// Mock pane for testing domain integration.
        struct MockPane {
            id: PaneId,
            domain_id: DomainId,
            alive: Mutex<bool>,
            exit_code: Mutex<Option<i32>>,
            output: Mutex<Vec<u8>>,
        }

        impl MockPane {
            fn new(domain_id: DomainId) -> Self {
                Self {
                    id: PaneId::new(),
                    domain_id,
                    alive: Mutex::new(true),
                    exit_code: Mutex::new(None),
                    output: Mutex::new(Vec::new()),
                }
            }
        }

        impl Pane for MockPane {
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
                let output = self.output.lock().unwrap();
                if output.is_empty() {
                    return Ok(0);
                }
                let len = output.len().min(buf.len());
                buf[..len].copy_from_slice(&output[..len]);
                Ok(len)
            }

            fn is_alive(&self) -> bool {
                *self.alive.lock().unwrap()
            }

            fn exit_status(&self) -> Option<i32> {
                *self.exit_code.lock().unwrap()
            }

            fn kill(&self) -> DomainResult<()> {
                *self.alive.lock().unwrap() = false;
                *self.exit_code.lock().unwrap() = Some(-9);
                Ok(())
            }
        }

        /// Mock domain for testing.
        struct MockDomain {
            id: DomainId,
            name: String,
            spawn_count: Mutex<usize>,
            should_fail: Mutex<bool>,
        }

        impl MockDomain {
            fn new(name: &str) -> Self {
                Self {
                    id: DomainId::new(),
                    name: name.to_string(),
                    spawn_count: Mutex::new(0),
                    should_fail: Mutex::new(false),
                }
            }

            fn set_should_fail(&self, fail: bool) {
                *self.should_fail.lock().unwrap() = fail;
            }

            fn spawn_count(&self) -> usize {
                *self.spawn_count.lock().unwrap()
            }
        }

        impl Domain for MockDomain {
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
                if *self.should_fail.lock().unwrap() {
                    return Err(DomainError::SpawnFailed("mock spawn failure".to_string()));
                }
                *self.spawn_count.lock().unwrap() += 1;
                Ok(Arc::new(MockPane::new(self.id)))
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
        fn test_begin_execution_without_domain() {
            // Test that execution works without domain (legacy behavior)
            let mut orch = create_orchestrator();
            assert!(!orch.has_domain_support());

            let agent_id = orch.spawn_agent(&[Capability::Shell]).unwrap();
            let cmd = Command::shell(CommandId(0), "echo hello");
            let cmd_id = orch.queue_command(cmd).unwrap();

            orch.assign_command(agent_id, cmd_id).unwrap();
            let exec_id = orch.begin_execution(agent_id).unwrap();

            // Execution started without domain
            assert!(orch.get_execution(exec_id).is_some());

            // Slot should NOT have resources (no domain configured)
            let exec = orch.get_execution(exec_id).unwrap();
            let slot = orch.terminal_pool.get(exec.terminal_id).unwrap();
            assert!(!slot.has_resources());
        }

        #[test]
        fn test_begin_execution_with_domain() {
            let mut orch = create_orchestrator();

            // Set up mock domain
            let domain = Arc::new(MockDomain::new("mock-local"));
            orch.set_default_domain(domain.clone());
            assert!(orch.has_domain_support());

            let agent_id = orch.spawn_agent(&[Capability::Shell]).unwrap();
            let cmd = Command::shell(CommandId(0), "echo hello");
            let cmd_id = orch.queue_command(cmd).unwrap();

            orch.assign_command(agent_id, cmd_id).unwrap();
            let exec_id = orch.begin_execution(agent_id).unwrap();

            // Verify pane was spawned
            assert_eq!(domain.spawn_count(), 1);

            // Execution should be running
            assert!(orch.get_execution(exec_id).is_some());

            // Slot should have resources attached
            let exec = orch.get_execution(exec_id).unwrap();
            let slot = orch.terminal_pool.get(exec.terminal_id).unwrap();
            assert!(slot.has_resources());
            assert!(slot.pane().is_some());
            assert!(slot.terminal().is_some());
            assert_eq!(slot.domain_id(), Some(domain.domain_id()));
        }

        #[test]
        fn test_begin_execution_spawn_failure() {
            let mut orch = create_orchestrator();

            // Set up mock domain that will fail
            let domain = Arc::new(MockDomain::new("failing-mock"));
            domain.set_should_fail(true);
            orch.set_default_domain(domain.clone());

            let agent_id = orch.spawn_agent(&[Capability::Shell]).unwrap();
            let cmd = Command::shell(CommandId(0), "echo hello");
            let cmd_id = orch.queue_command(cmd).unwrap();

            orch.assign_command(agent_id, cmd_id).unwrap();

            // Execution should fail
            let result = orch.begin_execution(agent_id);
            assert!(result.is_err());
            assert!(matches!(result, Err(OrchestratorError::SpawnFailed(_))));

            // Terminal should be released back to pool
            let (available, in_use) = orch.terminal_stats();
            assert_eq!(in_use, 0);
            assert!(available > 0);

            // Agent should still be in Assigned state (execution didn't start)
            let agent = orch.get_agent(agent_id).unwrap();
            assert_eq!(agent.state, AgentState::Assigned);
        }

        #[test]
        fn test_domain_registry_integration() {
            let mut orch = create_orchestrator();

            // Set up domain registry
            let registry = Arc::new(crate::domain::DomainRegistry::new());
            let domain = Arc::new(MockDomain::new("registry-mock"));
            registry.register(domain.clone());
            orch.set_domain_registry(registry);

            assert!(orch.has_domain_support());
            assert!(orch.domain_registry().is_some());

            let agent_id = orch.spawn_agent(&[Capability::Shell]).unwrap();
            let cmd = Command::shell(CommandId(0), "echo hello");
            let cmd_id = orch.queue_command(cmd).unwrap();

            orch.assign_command(agent_id, cmd_id).unwrap();
            let exec_id = orch.begin_execution(agent_id).unwrap();

            // Verify pane was spawned from registry domain
            assert_eq!(domain.spawn_count(), 1);
            assert!(orch.get_execution(exec_id).is_some());
        }

        #[test]
        fn test_default_domain_takes_precedence() {
            let mut orch = create_orchestrator();

            // Set up both registry and default domain
            let registry = Arc::new(crate::domain::DomainRegistry::new());
            let registry_domain = Arc::new(MockDomain::new("registry-mock"));
            registry.register(registry_domain.clone());

            let default_domain = Arc::new(MockDomain::new("default-mock"));

            orch.set_domain_registry(registry);
            orch.set_default_domain(default_domain.clone());

            let agent_id = orch.spawn_agent(&[Capability::Shell]).unwrap();
            let cmd = Command::shell(CommandId(0), "echo hello");
            let cmd_id = orch.queue_command(cmd).unwrap();

            orch.assign_command(agent_id, cmd_id).unwrap();
            let _exec_id = orch.begin_execution(agent_id).unwrap();

            // Default domain should be used (not registry)
            assert_eq!(default_domain.spawn_count(), 1);
            assert_eq!(registry_domain.spawn_count(), 0);
        }

        #[test]
        fn test_slot_resources_released_on_completion() {
            let mut orch = create_orchestrator();

            let domain = Arc::new(MockDomain::new("mock-local"));
            orch.set_default_domain(domain.clone());

            let agent_id = orch.spawn_agent(&[Capability::Shell]).unwrap();
            let cmd = Command::shell(CommandId(0), "echo hello");
            let cmd_id = orch.queue_command(cmd).unwrap();

            orch.assign_command(agent_id, cmd_id).unwrap();
            let exec_id = orch.begin_execution(agent_id).unwrap();

            // Get terminal ID before completion
            let terminal_id = orch.get_execution(exec_id).unwrap().terminal_id;

            // Complete execution
            orch.complete_execution(agent_id, 0).unwrap();

            // Slot should be available again with resources cleared
            let slot = orch.terminal_pool.get(terminal_id).unwrap();
            assert!(slot.is_available());
            assert!(!slot.has_resources());
            assert!(slot.pane().is_none());
            assert!(slot.terminal().is_none());
        }

        #[test]
        fn test_multiple_executions_with_domain() {
            let mut orch = create_orchestrator();

            let domain = Arc::new(MockDomain::new("mock-local"));
            orch.set_default_domain(domain.clone());

            // Spawn two agents
            let agent1 = orch.spawn_agent(&[Capability::Shell]).unwrap();
            let agent2 = orch.spawn_agent(&[Capability::Shell]).unwrap();

            // Queue two commands
            let cmd1 = Command::shell(CommandId(0), "cmd1");
            let cmd2 = Command::shell(CommandId(0), "cmd2");
            let cmd1_id = orch.queue_command(cmd1).unwrap();
            let cmd2_id = orch.queue_command(cmd2).unwrap();

            // Start both executions
            orch.assign_command(agent1, cmd1_id).unwrap();
            orch.assign_command(agent2, cmd2_id).unwrap();

            let exec1 = orch.begin_execution(agent1).unwrap();
            let exec2 = orch.begin_execution(agent2).unwrap();

            // Both should have spawned panes
            assert_eq!(domain.spawn_count(), 2);

            // Both slots should have resources
            let slot1 = orch
                .terminal_pool
                .get(orch.get_execution(exec1).unwrap().terminal_id)
                .unwrap();
            let slot2 = orch
                .terminal_pool
                .get(orch.get_execution(exec2).unwrap().terminal_id)
                .unwrap();

            assert!(slot1.has_resources());
            assert!(slot2.has_resources());

            // They should be different slots
            assert_ne!(slot1.id, slot2.id);
        }

        // =====================================================================
        // Phase 12 Step 4: Completion Detection Tests
        // =====================================================================

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
        fn test_check_completion_no_pane() {
            // Test completion detection without a pane (legacy mode)
            let mut orch = create_orchestrator();

            let agent_id = orch.spawn_agent(&[Capability::Shell]).unwrap();
            let cmd = Command::shell(CommandId(0), "echo hello");
            let cmd_id = orch.queue_command(cmd).unwrap();

            orch.assign_command(agent_id, cmd_id).unwrap();
            let exec_id = orch.begin_execution(agent_id).unwrap();

            // No pane attached, completion check should return None
            let result = orch.check_execution_completion(exec_id).unwrap();
            assert!(result.is_none());

            // Execution should still be running
            assert!(orch.get_execution(exec_id).unwrap().state.is_running());
        }

        #[test]
        fn test_check_completion_pane_still_alive() {
            let mut orch = create_orchestrator();

            let domain = Arc::new(ControllableMockDomain::new("controllable"));
            orch.set_default_domain(domain.clone());

            let agent_id = orch.spawn_agent(&[Capability::Shell]).unwrap();
            let cmd = Command::shell(CommandId(0), "echo hello");
            let cmd_id = orch.queue_command(cmd).unwrap();

            orch.assign_command(agent_id, cmd_id).unwrap();
            let exec_id = orch.begin_execution(agent_id).unwrap();

            // Pane is still alive, completion check should return None
            let result = orch.check_execution_completion(exec_id).unwrap();
            assert!(result.is_none());

            // Execution should still be running
            assert!(orch.get_execution(exec_id).unwrap().state.is_running());
        }

        #[test]
        fn test_check_completion_success() {
            let mut orch = create_orchestrator();

            let domain = Arc::new(ControllableMockDomain::new("controllable"));
            orch.set_default_domain(domain.clone());

            let agent_id = orch.spawn_agent(&[Capability::Shell]).unwrap();
            let cmd = Command::shell(CommandId(0), "echo hello");
            let cmd_id = orch.queue_command(cmd).unwrap();

            orch.assign_command(agent_id, cmd_id).unwrap();
            let exec_id = orch.begin_execution(agent_id).unwrap();

            // Simulate successful exit
            let pane = domain.get_last_pane().unwrap();
            pane.simulate_exit(0);

            // Completion check should detect success
            let result = orch.check_execution_completion(exec_id).unwrap();
            assert_eq!(result, Some((0, true)));

            // Execution should be succeeded
            let exec = orch.get_execution(exec_id).unwrap();
            assert_eq!(exec.state, crate::agent::ExecutionState::Succeeded);
            assert_eq!(exec.exit_code, Some(0));

            // Agent should be completed
            let agent = orch.get_agent(agent_id).unwrap();
            assert_eq!(agent.state, AgentState::Completed);

            // Terminal slot should be released
            let terminal_id = exec.terminal_id;
            let slot = orch.terminal_pool.get(terminal_id).unwrap();
            assert!(slot.is_available());
            assert!(!slot.has_resources());
        }

        #[test]
        fn test_check_completion_failure() {
            let mut orch = create_orchestrator();

            let domain = Arc::new(ControllableMockDomain::new("controllable"));
            orch.set_default_domain(domain.clone());

            let agent_id = orch.spawn_agent(&[Capability::Shell]).unwrap();
            let cmd = Command::shell(CommandId(0), "false");
            let cmd_id = orch.queue_command(cmd).unwrap();

            orch.assign_command(agent_id, cmd_id).unwrap();
            let exec_id = orch.begin_execution(agent_id).unwrap();

            // Simulate failed exit
            let pane = domain.get_last_pane().unwrap();
            pane.simulate_exit(1);

            // Completion check should detect failure
            let result = orch.check_execution_completion(exec_id).unwrap();
            assert_eq!(result, Some((1, false)));

            // Execution should be failed with exit code stored
            let exec = orch.get_execution(exec_id).unwrap();
            assert_eq!(exec.state, crate::agent::ExecutionState::Failed);
            assert!(exec.error.is_some());
            assert_eq!(exec.exit_code, Some(1));

            // Agent should be failed
            let agent = orch.get_agent(agent_id).unwrap();
            assert_eq!(agent.state, AgentState::Failed);
        }

        #[test]
        fn test_check_all_completions() {
            let mut orch = create_orchestrator();

            let domain = Arc::new(ControllableMockDomain::new("controllable"));
            orch.set_default_domain(domain.clone());

            // Spawn two agents and start executions
            let agent1 = orch.spawn_agent(&[Capability::Shell]).unwrap();
            let agent2 = orch.spawn_agent(&[Capability::Shell]).unwrap();

            let cmd1_id = orch
                .queue_command(Command::shell(CommandId(0), "cmd1"))
                .unwrap();
            let cmd2_id = orch
                .queue_command(Command::shell(CommandId(0), "cmd2"))
                .unwrap();

            orch.assign_command(agent1, cmd1_id).unwrap();
            orch.assign_command(agent2, cmd2_id).unwrap();

            orch.begin_execution(agent1).unwrap();
            orch.begin_execution(agent2).unwrap();

            // Both still running
            assert_eq!(orch.active_execution_count(), 2);

            // Simulate exit of first pane
            let panes: Vec<_> = domain.spawned_panes.lock().unwrap().clone();
            panes[0].simulate_exit(0);

            // Check all completions
            let completed = orch.check_all_completions();
            assert_eq!(completed, 1);
            assert_eq!(orch.active_execution_count(), 1);

            // Simulate exit of second pane with error
            panes[1].simulate_exit(127);

            let completed = orch.check_all_completions();
            assert_eq!(completed, 1);
            assert_eq!(orch.active_execution_count(), 0);
        }

        #[test]
        fn test_poll_executions_with_output() {
            let mut orch = create_orchestrator();

            let domain = Arc::new(ControllableMockDomain::new("controllable"));
            orch.set_default_domain(domain.clone());

            let agent_id = orch.spawn_agent(&[Capability::Shell]).unwrap();
            let cmd = Command::shell(CommandId(0), "echo hello");
            let cmd_id = orch.queue_command(cmd).unwrap();

            orch.assign_command(agent_id, cmd_id).unwrap();
            let exec_id = orch.begin_execution(agent_id).unwrap();

            // Add output to pane
            let pane = domain.get_last_pane().unwrap();
            pane.add_output(b"hello world\n");

            // Poll executions (should read output)
            let completed = orch.poll_executions();
            assert_eq!(completed, 0); // Still running

            // Check that output was captured
            let exec = orch.get_execution(exec_id).unwrap();
            assert_eq!(exec.stdout, b"hello world\n");

            // Simulate exit
            pane.simulate_exit(0);

            // Poll again - should detect completion
            let completed = orch.poll_executions();
            assert_eq!(completed, 1);

            // Execution should be complete
            let exec = orch.get_execution(exec_id).unwrap();
            assert!(!exec.state.is_running());
        }

        #[test]
        fn test_check_completion_nonexistent_execution() {
            let mut orch = create_orchestrator();

            let result = orch.check_execution_completion(ExecutionId(999));
            assert!(matches!(
                result,
                Err(OrchestratorError::ExecutionNotFound(_))
            ));
        }

        #[test]
        fn test_check_completion_already_completed() {
            let mut orch = create_orchestrator();

            let domain = Arc::new(ControllableMockDomain::new("controllable"));
            orch.set_default_domain(domain.clone());

            let agent_id = orch.spawn_agent(&[Capability::Shell]).unwrap();
            let cmd = Command::shell(CommandId(0), "echo hello");
            let cmd_id = orch.queue_command(cmd).unwrap();

            orch.assign_command(agent_id, cmd_id).unwrap();
            let exec_id = orch.begin_execution(agent_id).unwrap();

            // Simulate exit
            let pane = domain.get_last_pane().unwrap();
            pane.simulate_exit(0);

            // First check detects completion
            let result = orch.check_execution_completion(exec_id).unwrap();
            assert_eq!(result, Some((0, true)));

            // Second check on already-completed execution returns None
            let result = orch.check_execution_completion(exec_id).unwrap();
            assert!(result.is_none());
        }
    }
}
