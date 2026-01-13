//! Terminal pool management.
//!
//! Implements the Terminal record from AgentOrchestration.tla:
//! ```text
//! Terminal == [
//!     id: Nat,
//!     state: TerminalStates,
//!     currentExecutionId: Nat ∪ {-1}
//! ]
//! ```
//!
//! Terminal slots are pre-allocated resources that agents can use for execution.
//! This enforces the INV-ORCH-3 invariant: terminal used by at most one execution.
//!
//! ## Phase 12: Real Terminal Resources
//!
//! Terminal slots can optionally hold real terminal resources:
//! - `pane: Option<Arc<dyn Pane>>` - The pane spawned from a domain
//! - `terminal: Option<Terminal>` - The terminal state machine for parsing output
//! - `domain_id: Option<DomainId>` - The domain that owns the pane
//!
//! These are set when the orchestrator spawns a pane for execution.

use std::fmt;
use std::sync::Arc;

use super::ExecutionId;
use crate::domain::{DomainId, Pane};
use crate::terminal::Terminal;

/// Unique identifier for a terminal slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalSlotId(pub u64);

impl fmt::Display for TerminalSlotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Term({})", self.0)
    }
}

/// Terminal slot states.
///
/// Maps to `TerminalStates` in TLA+ spec:
/// ```text
/// TerminalStates == {"Available", "InUse", "Closed"}
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerminalSlotState {
    /// Slot is available for allocation
    Available,
    /// Slot is in use by an execution
    InUse,
    /// Slot has been closed and cannot be used
    Closed,
}

impl fmt::Display for TerminalSlotState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            TerminalSlotState::Available => "Available",
            TerminalSlotState::InUse => "InUse",
            TerminalSlotState::Closed => "Closed",
        };
        write!(f, "{}", name)
    }
}

/// A terminal slot that can be allocated for command execution.
///
/// Implements the Terminal record from the TLA+ specification.
///
/// ## Phase 12: Real Terminal Resources
///
/// When integrated with a domain, slots hold real resources:
/// - `pane` - The pane spawned from a domain for I/O
/// - `terminal` - The terminal state machine for parsing output
/// - `domain_id` - The domain that owns the pane
pub struct TerminalSlot {
    /// Unique identifier
    pub id: TerminalSlotId,
    /// Current state
    pub state: TerminalSlotState,
    /// Execution currently using this slot (None = -1 in TLA+)
    pub current_execution_id: Option<ExecutionId>,
    /// The pane for I/O (Phase 12)
    pane: Option<Arc<dyn Pane>>,
    /// The terminal state machine (Phase 12)
    terminal: Option<Terminal>,
    /// The domain that owns the pane (Phase 12)
    domain_id: Option<DomainId>,
}

impl fmt::Debug for TerminalSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TerminalSlot")
            .field("id", &self.id)
            .field("state", &self.state)
            .field("current_execution_id", &self.current_execution_id)
            .field("has_pane", &self.pane.is_some())
            .field("has_terminal", &self.terminal.is_some())
            .field("domain_id", &self.domain_id)
            .finish()
    }
}

impl Clone for TerminalSlot {
    fn clone(&self) -> Self {
        // Note: pane and terminal are NOT cloned - only metadata is preserved.
        // This is intentional: real resources should not be cloned.
        Self {
            id: self.id,
            state: self.state,
            current_execution_id: self.current_execution_id,
            pane: None,
            terminal: None,
            domain_id: self.domain_id,
        }
    }
}

impl TerminalSlot {
    /// Create a new available terminal slot.
    pub fn new(id: TerminalSlotId) -> Self {
        Self {
            id,
            state: TerminalSlotState::Available,
            current_execution_id: None,
            pane: None,
            terminal: None,
            domain_id: None,
        }
    }

    /// Check if this slot is available for allocation.
    pub fn is_available(&self) -> bool {
        matches!(self.state, TerminalSlotState::Available)
    }

    /// Check if this slot is in use.
    pub fn is_in_use(&self) -> bool {
        matches!(self.state, TerminalSlotState::InUse)
    }

    /// Allocate this slot for an execution.
    ///
    /// # Preconditions
    /// - Slot must be Available
    pub fn allocate(&mut self, execution_id: ExecutionId) -> Result<(), &'static str> {
        if self.state != TerminalSlotState::Available {
            return Err("Terminal slot must be Available to allocate");
        }
        self.state = TerminalSlotState::InUse;
        self.current_execution_id = Some(execution_id);
        Ok(())
    }

    /// Release this slot back to the pool.
    ///
    /// Clears all resources (pane, terminal, domain_id).
    ///
    /// # Preconditions
    /// - Slot must be InUse
    pub fn release(&mut self) -> Result<(), &'static str> {
        if self.state != TerminalSlotState::InUse {
            return Err("Terminal slot must be InUse to release");
        }
        self.state = TerminalSlotState::Available;
        self.current_execution_id = None;
        // Clear Phase 12 resources
        self.pane = None;
        self.terminal = None;
        self.domain_id = None;
        Ok(())
    }

    /// Close this slot permanently.
    ///
    /// # Preconditions
    /// - Slot must be Available (cannot close in-use slot)
    pub fn close(&mut self) -> Result<(), &'static str> {
        if self.state != TerminalSlotState::Available {
            return Err("Terminal slot must be Available to close");
        }
        self.state = TerminalSlotState::Closed;
        Ok(())
    }

    // =========================================================================
    // Phase 12: Real Terminal Resources
    // =========================================================================

    /// Attach a pane to this slot.
    ///
    /// Called by the orchestrator when spawning a pane for execution.
    pub fn attach_pane(&mut self, pane: Arc<dyn Pane>, domain_id: DomainId) {
        self.pane = Some(pane);
        self.domain_id = Some(domain_id);
    }

    /// Attach a terminal state machine to this slot.
    ///
    /// Called by the orchestrator when setting up execution.
    pub fn attach_terminal(&mut self, terminal: Terminal) {
        self.terminal = Some(terminal);
    }

    /// Get the pane (if attached).
    pub fn pane(&self) -> Option<&Arc<dyn Pane>> {
        self.pane.as_ref()
    }

    /// Get the terminal (if attached).
    pub fn terminal(&self) -> Option<&Terminal> {
        self.terminal.as_ref()
    }

    /// Get mutable access to the terminal (if attached).
    pub fn terminal_mut(&mut self) -> Option<&mut Terminal> {
        self.terminal.as_mut()
    }

    /// Get the domain ID (if attached).
    pub fn domain_id(&self) -> Option<DomainId> {
        self.domain_id
    }

    /// Check if this slot has real resources attached.
    pub fn has_resources(&self) -> bool {
        self.pane.is_some()
    }
}

/// Pool of terminal slots for agent execution.
///
/// Implements the terminals function from TLA+ spec with pre-allocation.
#[derive(Debug)]
pub struct TerminalPool {
    /// All terminal slots
    slots: Vec<TerminalSlot>,
}

impl TerminalPool {
    /// Create a new terminal pool with the given number of slots.
    ///
    /// All slots are initialized as Available.
    pub fn new(size: usize) -> Self {
        let slots = (0..size)
            .map(|i| TerminalSlot::new(TerminalSlotId(i as u64)))
            .collect();
        Self { slots }
    }

    /// Get the total pool size.
    pub fn size(&self) -> usize {
        self.slots.len()
    }

    /// Get the number of available slots.
    pub fn available_count(&self) -> usize {
        self.slots.iter().filter(|s| s.is_available()).count()
    }

    /// Get the number of in-use slots.
    pub fn in_use_count(&self) -> usize {
        self.slots.iter().filter(|s| s.is_in_use()).count()
    }

    /// Check if any slots are available.
    pub fn has_available(&self) -> bool {
        self.slots.iter().any(|s| s.is_available())
    }

    /// Allocate a terminal slot for an execution.
    ///
    /// Implements `AvailableTerminals` selection from TLA+ spec.
    ///
    /// # Returns
    /// - `Ok(TerminalSlotId)` if a slot was allocated
    /// - `Err(&'static str)` if no slots available
    pub fn allocate(&mut self, execution_id: ExecutionId) -> Result<TerminalSlotId, &'static str> {
        let slot = self
            .slots
            .iter_mut()
            .find(|s| s.is_available())
            .ok_or("No terminal slots available")?;
        slot.allocate(execution_id)?;
        Ok(slot.id)
    }

    /// Release a terminal slot back to the pool.
    pub fn release(&mut self, slot_id: TerminalSlotId) -> Result<(), &'static str> {
        let slot = self.get_mut(slot_id).ok_or("Terminal slot not found")?;
        slot.release()
    }

    /// Close a terminal slot.
    pub fn close(&mut self, slot_id: TerminalSlotId) -> Result<(), &'static str> {
        let slot = self.get_mut(slot_id).ok_or("Terminal slot not found")?;
        slot.close()
    }

    /// Get a slot by ID.
    pub fn get(&self, id: TerminalSlotId) -> Option<&TerminalSlot> {
        self.slots.iter().find(|s| s.id == id)
    }

    /// Get a mutable reference to a slot by ID.
    pub fn get_mut(&mut self, id: TerminalSlotId) -> Option<&mut TerminalSlot> {
        self.slots.iter_mut().find(|s| s.id == id)
    }

    /// Find the slot allocated to an execution.
    pub fn find_by_execution(&self, execution_id: ExecutionId) -> Option<&TerminalSlot> {
        self.slots
            .iter()
            .find(|s| s.current_execution_id == Some(execution_id))
    }

    /// Get all available slots.
    pub fn available(&self) -> impl Iterator<Item = &TerminalSlot> {
        self.slots.iter().filter(|s| s.is_available())
    }

    /// Get all in-use slots.
    pub fn in_use(&self) -> impl Iterator<Item = &TerminalSlot> {
        self.slots.iter().filter(|s| s.is_in_use())
    }

    /// Get all slot IDs.
    pub fn slot_ids(&self) -> impl Iterator<Item = TerminalSlotId> + '_ {
        self.slots.iter().map(|s| s.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_slot_lifecycle() {
        let mut slot = TerminalSlot::new(TerminalSlotId(0));

        assert!(slot.is_available());
        assert!(!slot.is_in_use());

        // Allocate
        slot.allocate(ExecutionId(1)).unwrap();
        assert!(!slot.is_available());
        assert!(slot.is_in_use());
        assert_eq!(slot.current_execution_id, Some(ExecutionId(1)));

        // Release
        slot.release().unwrap();
        assert!(slot.is_available());
        assert_eq!(slot.current_execution_id, None);

        // Close
        slot.close().unwrap();
        assert_eq!(slot.state, TerminalSlotState::Closed);
    }

    #[test]
    fn test_terminal_slot_invalid_transitions() {
        let mut slot = TerminalSlot::new(TerminalSlotId(0));

        // Cannot release available slot
        assert!(slot.release().is_err());

        // Cannot close in-use slot
        slot.allocate(ExecutionId(1)).unwrap();
        assert!(slot.close().is_err());

        // Cannot double-allocate
        assert!(slot.allocate(ExecutionId(2)).is_err());
    }

    #[test]
    fn test_terminal_pool_basic() {
        let pool = TerminalPool::new(5);

        assert_eq!(pool.size(), 5);
        assert_eq!(pool.available_count(), 5);
        assert_eq!(pool.in_use_count(), 0);
        assert!(pool.has_available());
    }

    #[test]
    fn test_terminal_pool_allocation() {
        let mut pool = TerminalPool::new(3);

        // Allocate all slots
        let id1 = pool.allocate(ExecutionId(1)).unwrap();
        let id2 = pool.allocate(ExecutionId(2)).unwrap();
        let id3 = pool.allocate(ExecutionId(3)).unwrap();

        assert_eq!(pool.available_count(), 0);
        assert_eq!(pool.in_use_count(), 3);
        assert!(!pool.has_available());

        // Fourth allocation fails
        assert!(pool.allocate(ExecutionId(4)).is_err());

        // Release one
        pool.release(id2).unwrap();
        assert_eq!(pool.available_count(), 1);
        assert!(pool.has_available());

        // Can allocate again
        let id4 = pool.allocate(ExecutionId(4)).unwrap();
        assert_eq!(id4, id2); // Same slot reused

        // Verify slots are distinct
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_terminal_pool_find() {
        let mut pool = TerminalPool::new(3);

        let id = pool.allocate(ExecutionId(42)).unwrap();

        // Find by execution
        let slot = pool.find_by_execution(ExecutionId(42)).unwrap();
        assert_eq!(slot.id, id);
        assert_eq!(slot.current_execution_id, Some(ExecutionId(42)));

        // Find by ID
        let slot = pool.get(id).unwrap();
        assert_eq!(slot.current_execution_id, Some(ExecutionId(42)));

        // Not found
        assert!(pool.find_by_execution(ExecutionId(99)).is_none());
    }

    #[test]
    fn test_terminal_pool_close() {
        let mut pool = TerminalPool::new(3);

        // Close an available slot
        let id = TerminalSlotId(0);
        pool.close(id).unwrap();

        assert_eq!(pool.available_count(), 2); // One less available

        // Cannot allocate closed slot
        let slot = pool.get(id).unwrap();
        assert_eq!(slot.state, TerminalSlotState::Closed);
    }
}
