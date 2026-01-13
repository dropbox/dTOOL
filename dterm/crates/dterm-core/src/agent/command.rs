//! Command types and queue management.
//!
//! Implements the Command record from AgentOrchestration.tla:
//! ```text
//! Command == [
//!     id: Nat,
//!     commandType: CommandTypes,
//!     requiredCapabilities: SUBSET Capabilities,
//!     dependencies: SUBSET Nat,
//!     approved: BOOLEAN
//! ]
//! ```

use std::collections::{HashSet, VecDeque};
use std::fmt;

use super::Capability;

/// Unique identifier for a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommandId(pub u64);

impl fmt::Display for CommandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Cmd({})", self.0)
    }
}

/// Types of commands that can be executed.
///
/// Maps to `CommandTypes` constant in TLA+ spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandType {
    /// Shell command execution (e.g., running a script)
    Shell,
    /// File operation (read, write, delete)
    FileOp,
    /// Network request
    Network,
    /// Git operation
    Git,
    /// Package manager operation
    Package,
    /// Container operation (docker, podman)
    Container,
    /// Database query
    Database,
    /// System administration task
    Admin,
}

impl CommandType {
    /// Get the minimum required capability for this command type.
    pub fn required_capability(&self) -> Capability {
        match self {
            CommandType::Shell => Capability::Shell,
            CommandType::FileOp => Capability::File,
            CommandType::Network => Capability::Net,
            CommandType::Git => Capability::Git,
            CommandType::Package => Capability::Package,
            CommandType::Container => Capability::Container,
            CommandType::Database => Capability::Database,
            CommandType::Admin => Capability::Admin,
        }
    }
}

impl fmt::Display for CommandType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            CommandType::Shell => "shell",
            CommandType::FileOp => "file",
            CommandType::Network => "network",
            CommandType::Git => "git",
            CommandType::Package => "package",
            CommandType::Container => "container",
            CommandType::Database => "database",
            CommandType::Admin => "admin",
        };
        write!(f, "{}", name)
    }
}

/// A command to be executed by an agent.
///
/// Implements the Command record from the TLA+ specification.
#[derive(Debug, Clone)]
pub struct Command {
    /// Unique identifier
    pub id: CommandId,
    /// Type of command
    pub command_type: CommandType,
    /// Required capabilities to execute this command
    pub required_capabilities: HashSet<Capability>,
    /// IDs of commands that must complete before this one
    pub dependencies: HashSet<CommandId>,
    /// Whether this command has been approved for execution
    pub approved: bool,
    /// Human-readable description
    pub description: String,
    /// The actual command/action to execute
    pub payload: String,
}

impl Command {
    /// Create a new command builder.
    pub fn builder(command_type: CommandType) -> CommandBuilder {
        CommandBuilder::new(command_type)
    }

    /// Create a simple shell command (pre-approved, no dependencies).
    pub fn shell(id: CommandId, payload: &str) -> Self {
        Self {
            id,
            command_type: CommandType::Shell,
            required_capabilities: [Capability::Shell].into_iter().collect(),
            dependencies: HashSet::new(),
            approved: true,
            description: format!("Shell: {}", payload),
            payload: payload.to_string(),
        }
    }
}

/// Builder for constructing commands.
#[must_use]
pub struct CommandBuilder {
    command_type: CommandType,
    required_capabilities: HashSet<Capability>,
    dependencies: HashSet<CommandId>,
    approved: bool,
    description: String,
    payload: String,
}

impl CommandBuilder {
    /// Create a new command builder.
    pub fn new(command_type: CommandType) -> Self {
        let mut required_capabilities = HashSet::new();
        required_capabilities.insert(command_type.required_capability());
        Self {
            command_type,
            required_capabilities,
            dependencies: HashSet::new(),
            approved: false,
            description: String::new(),
            payload: String::new(),
        }
    }

    /// Add a required capability.
    pub fn require(mut self, cap: Capability) -> Self {
        self.required_capabilities.insert(cap);
        self
    }

    /// Add a dependency on another command.
    pub fn depends_on(mut self, cmd_id: CommandId) -> Self {
        self.dependencies.insert(cmd_id);
        self
    }

    /// Mark as pre-approved.
    pub fn approved(mut self) -> Self {
        self.approved = true;
        self
    }

    /// Set description.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set payload.
    pub fn payload(mut self, payload: impl Into<String>) -> Self {
        self.payload = payload.into();
        self
    }

    /// Build the command with the given ID.
    pub fn build(self, id: CommandId) -> Command {
        Command {
            id,
            command_type: self.command_type,
            required_capabilities: self.required_capabilities,
            dependencies: self.dependencies,
            approved: self.approved,
            description: self.description,
            payload: self.payload,
        }
    }
}

// ============================================================================
// Command Queue Error
// ============================================================================

/// Errors from command queue operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandQueueError {
    /// Queue is full
    QueueFull,
    /// Command not found
    CommandNotFound(CommandId),
    /// Command already approved
    AlreadyApproved(CommandId),
}

impl fmt::Display for CommandQueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandQueueError::QueueFull => write!(f, "Command queue is full"),
            CommandQueueError::CommandNotFound(id) => write!(f, "Command not found: {}", id),
            CommandQueueError::AlreadyApproved(id) => write!(f, "Command {} already approved", id),
        }
    }
}

impl std::error::Error for CommandQueueError {}

/// Result type for command queue operations.
pub type CommandQueueResult<T> = Result<T, CommandQueueError>;

/// Queue of commands awaiting assignment and execution.
///
/// Implements `commandQueue` from TLA+ spec with maximum size enforcement.
#[derive(Debug)]
pub struct CommandQueue {
    /// Commands waiting to be assigned
    queue: VecDeque<Command>,
    /// Maximum queue size (TLA+: MaxCommands)
    max_size: usize,
    /// Next command ID to assign
    next_id: u64,
}

impl CommandQueue {
    /// Create a new command queue with the given maximum size.
    pub fn new(max_size: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            max_size,
            next_id: 0,
        }
    }

    /// Get the number of commands in the queue.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Check if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Check if the queue is full.
    pub fn is_full(&self) -> bool {
        self.queue.len() >= self.max_size
    }

    /// Generate a new unique command ID.
    pub fn next_command_id(&mut self) -> CommandId {
        let id = CommandId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Add a command to the queue.
    ///
    /// # Returns
    /// - `Ok(CommandId)` if command was added
    /// - `Err(CommandQueueError::QueueFull)` if queue is full
    pub fn enqueue(&mut self, mut command: Command) -> CommandQueueResult<CommandId> {
        if self.is_full() {
            return Err(CommandQueueError::QueueFull);
        }
        let id = self.next_command_id();
        command.id = id;
        self.queue.push_back(command);
        Ok(id)
    }

    /// Get a command by ID.
    pub fn get(&self, id: CommandId) -> Option<&Command> {
        self.queue.iter().find(|c| c.id == id)
    }

    /// Get a mutable reference to a command by ID.
    pub fn get_mut(&mut self, id: CommandId) -> Option<&mut Command> {
        self.queue.iter_mut().find(|c| c.id == id)
    }

    /// Remove a command from the queue by ID.
    pub fn remove(&mut self, id: CommandId) -> Option<Command> {
        let pos = self.queue.iter().position(|c| c.id == id)?;
        self.queue.remove(pos)
    }

    /// Approve a command by ID.
    ///
    /// Implements `ApproveCommand` from TLA+ spec.
    pub fn approve(&mut self, id: CommandId) -> CommandQueueResult<()> {
        let cmd = self
            .get_mut(id)
            .ok_or(CommandQueueError::CommandNotFound(id))?;
        if cmd.approved {
            return Err(CommandQueueError::AlreadyApproved(id));
        }
        cmd.approved = true;
        Ok(())
    }

    /// Get commands that are ready for assignment.
    ///
    /// Implements `ReadyCommands` from TLA+ spec:
    /// - Approved
    /// - Dependencies satisfied
    /// - Not currently assigned
    pub fn ready_commands<'a>(
        &'a self,
        completed_commands: &'a HashSet<CommandId>,
        assigned_commands: &'a HashSet<CommandId>,
    ) -> impl Iterator<Item = &'a Command> + 'a {
        self.queue.iter().filter(move |cmd| {
            cmd.approved
                && cmd.dependencies.is_subset(completed_commands)
                && !assigned_commands.contains(&cmd.id)
        })
    }

    /// Iterate over all commands.
    pub fn iter(&self) -> impl Iterator<Item = &Command> {
        self.queue.iter()
    }

    /// Get all command IDs currently in the queue.
    pub fn command_ids(&self) -> HashSet<CommandId> {
        self.queue.iter().map(|c| c.id).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_builder() {
        let cmd = Command::builder(CommandType::Shell)
            .require(Capability::File)
            .depends_on(CommandId(1))
            .approved()
            .description("Test command")
            .payload("echo hello")
            .build(CommandId(2));

        assert_eq!(cmd.id, CommandId(2));
        assert_eq!(cmd.command_type, CommandType::Shell);
        assert!(cmd.required_capabilities.contains(&Capability::Shell));
        assert!(cmd.required_capabilities.contains(&Capability::File));
        assert!(cmd.dependencies.contains(&CommandId(1)));
        assert!(cmd.approved);
        assert_eq!(cmd.description, "Test command");
        assert_eq!(cmd.payload, "echo hello");
    }

    #[test]
    fn test_command_queue_basic() {
        let mut queue = CommandQueue::new(10);
        assert!(queue.is_empty());

        let cmd = Command::shell(CommandId(0), "echo hello");
        let id = queue.enqueue(cmd).unwrap();

        assert_eq!(queue.len(), 1);
        assert!(!queue.is_empty());
        assert!(queue.get(id).is_some());
    }

    #[test]
    fn test_command_queue_full() {
        let mut queue = CommandQueue::new(2);

        queue.enqueue(Command::shell(CommandId(0), "cmd1")).unwrap();
        queue.enqueue(Command::shell(CommandId(0), "cmd2")).unwrap();

        assert!(queue.is_full());
        assert!(queue.enqueue(Command::shell(CommandId(0), "cmd3")).is_err());
    }

    #[test]
    fn test_command_approval() {
        let mut queue = CommandQueue::new(10);

        let cmd = Command::builder(CommandType::Shell)
            .payload("echo")
            .build(CommandId(0));
        let id = queue.enqueue(cmd).unwrap();

        assert!(!queue.get(id).unwrap().approved);
        queue.approve(id).unwrap();
        assert!(queue.get(id).unwrap().approved);

        // Double approval fails
        assert!(queue.approve(id).is_err());
    }

    #[test]
    fn test_ready_commands() {
        let mut queue = CommandQueue::new(10);

        // Unapproved command
        let cmd1 = Command::builder(CommandType::Shell)
            .payload("unapproved")
            .build(CommandId(0));
        let id1 = queue.enqueue(cmd1).unwrap();

        // Approved command with no dependencies
        let cmd2 = Command::builder(CommandType::Shell)
            .approved()
            .payload("ready")
            .build(CommandId(0));
        let id2 = queue.enqueue(cmd2).unwrap();

        // Approved command with unsatisfied dependency
        let cmd3 = Command::builder(CommandType::Shell)
            .approved()
            .depends_on(CommandId(99))
            .payload("blocked")
            .build(CommandId(0));
        let _id3 = queue.enqueue(cmd3).unwrap();

        let completed = HashSet::new();
        let assigned = HashSet::new();

        let ready: Vec<_> = queue.ready_commands(&completed, &assigned).collect();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, id2);

        // After approval
        queue.approve(id1).unwrap();
        let ready: Vec<_> = queue.ready_commands(&completed, &assigned).collect();
        assert_eq!(ready.len(), 2);
    }

    #[test]
    fn test_command_removal() {
        let mut queue = CommandQueue::new(10);

        let id1 = queue.enqueue(Command::shell(CommandId(0), "cmd1")).unwrap();
        let id2 = queue.enqueue(Command::shell(CommandId(0), "cmd2")).unwrap();

        assert_eq!(queue.len(), 2);

        let removed = queue.remove(id1);
        assert!(removed.is_some());
        assert_eq!(queue.len(), 1);
        assert!(queue.get(id1).is_none());
        assert!(queue.get(id2).is_some());
    }
}
