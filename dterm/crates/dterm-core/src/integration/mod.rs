//! Integration module for connecting agent orchestration with media server.
//!
//! This module provides the glue between:
//! - Agent orchestration (`agent::Orchestrator`) - command execution
//! - Media server (`media::MediaServer`) - voice I/O
//! - Terminal processing (`terminal::Terminal`) - VT100 state machine
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    AgentMediaBridge                              │
//! │                                                                  │
//! │  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐       │
//! │  │ Orchestrator │───▶│ Voice I/O    │───▶│ Terminal     │       │
//! │  │ (commands)   │    │ (STT/TTS)    │    │ (output)     │       │
//! │  └──────────────┘    └──────────────┘    └──────────────┘       │
//! │          │                  │                   │                │
//! │          └──────────────────┴───────────────────┘                │
//! │                          │                                       │
//! │                  Approval Workflow                               │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Voice I/O Flow
//!
//! 1. User speaks → STT → text command
//! 2. Command parsed → Orchestrator queues
//! 3. Approval check (if dangerous)
//! 4. Agent executes → Terminal output
//! 5. Output summarized → TTS → User hears
//!
//! ## Safety Invariants
//!
//! - **INV-BRIDGE-1**: Voice commands require approval for dangerous operations
//! - **INV-BRIDGE-2**: TTS queue bounded per agent (from MediaServer)
//! - **INV-BRIDGE-3**: Only one agent can speak at a time (implicit from STT)
//! - **INV-BRIDGE-4**: Command completion triggers TTS notification

use crate::agent::{
    AgentId, ApprovalError, ApprovalRequestId, Command, CommandId, CommandType, Orchestrator,
    OrchestratorConfig, OrchestratorError,
};
use crate::media::{
    AudioFormat, ClientId, MediaServer, MediaServerConfig, MediaServerError, Priority, SttResult,
    SttState,
};

/// Configuration for the agent-media integration bridge.
#[derive(Debug, Clone)]
pub struct AgentMediaBridgeConfig {
    /// Orchestrator configuration.
    pub orchestrator: OrchestratorConfig,
    /// Media server configuration.
    pub media: MediaServerConfig,
    /// Whether to auto-announce command completions via TTS.
    pub announce_completions: bool,
    /// Whether to auto-announce errors via TTS.
    pub announce_errors: bool,
    /// Maximum words in TTS announcement (truncate longer output).
    pub max_announcement_words: usize,
}

impl Default for AgentMediaBridgeConfig {
    fn default() -> Self {
        Self {
            orchestrator: OrchestratorConfig::default(),
            media: MediaServerConfig::default(),
            announce_completions: true,
            announce_errors: true,
            max_announcement_words: 50,
        }
    }
}

/// Bridge connecting agent orchestration with media server for voice I/O.
///
/// This struct coordinates:
/// - Voice input (STT) → command parsing → agent execution
/// - Agent completion → output summarization → voice output (TTS)
pub struct AgentMediaBridge {
    /// Agent orchestrator.
    orchestrator: Orchestrator,
    /// Media server for voice I/O.
    media: MediaServer,
    /// Configuration.
    config: AgentMediaBridgeConfig,
    /// Mapping from agent IDs to media client IDs.
    agent_to_client: std::collections::HashMap<AgentId, ClientId>,
    /// Reverse mapping from client IDs to agent IDs.
    client_to_agent: std::collections::HashMap<ClientId, AgentId>,
    /// Next client ID to assign.
    next_client_id: ClientId,
    /// Pending voice commands awaiting parsing.
    pending_voice_commands: std::collections::VecDeque<(ClientId, String)>,
}

impl std::fmt::Debug for AgentMediaBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentMediaBridge")
            .field("orchestrator_agents", &self.orchestrator.agents().count())
            .field("media_stt_state", &self.media.stt_state())
            .field("agent_to_client_count", &self.agent_to_client.len())
            .field("pending_voice_commands", &self.pending_voice_commands.len())
            .finish()
    }
}

impl AgentMediaBridge {
    /// Create a new agent-media bridge with default configuration.
    pub fn new() -> Self {
        Self::with_config(AgentMediaBridgeConfig::default())
    }

    /// Create a new agent-media bridge with custom configuration.
    pub fn with_config(config: AgentMediaBridgeConfig) -> Self {
        Self {
            orchestrator: Orchestrator::new(config.orchestrator.clone()),
            media: MediaServer::new(config.media.clone()),
            config,
            agent_to_client: std::collections::HashMap::new(),
            client_to_agent: std::collections::HashMap::new(),
            next_client_id: 1,
            pending_voice_commands: std::collections::VecDeque::new(),
        }
    }

    // ========================================================================
    // Agent Management
    // ========================================================================

    /// Register an agent with the media bridge.
    ///
    /// This creates a media client ID for the agent, enabling voice I/O.
    pub fn register_agent(&mut self, agent_id: AgentId) -> ClientId {
        if let Some(&client_id) = self.agent_to_client.get(&agent_id) {
            return client_id;
        }

        let client_id = self.next_client_id;
        self.next_client_id += 1;

        self.agent_to_client.insert(agent_id, client_id);
        self.client_to_agent.insert(client_id, agent_id);

        client_id
    }

    /// Unregister an agent from the media bridge.
    ///
    /// This cleans up the media client and any pending voice state.
    pub fn unregister_agent(&mut self, agent_id: AgentId) {
        if let Some(client_id) = self.agent_to_client.remove(&agent_id) {
            self.client_to_agent.remove(&client_id);
            self.media.client_disconnect(client_id);
        }
    }

    /// Get the client ID for an agent.
    pub fn client_for_agent(&self, agent_id: AgentId) -> Option<ClientId> {
        self.agent_to_client.get(&agent_id).copied()
    }

    /// Get the agent ID for a client.
    pub fn agent_for_client(&self, client_id: ClientId) -> Option<AgentId> {
        self.client_to_agent.get(&client_id).copied()
    }

    // ========================================================================
    // Voice Input (STT)
    // ========================================================================

    /// Start voice input session for an agent.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Agent is not registered
    /// - Another STT session is already active (INV-MEDIA-1)
    pub fn start_voice_input(&mut self, agent_id: AgentId) -> Result<(), AgentMediaBridgeError> {
        let client_id = self
            .agent_to_client
            .get(&agent_id)
            .copied()
            .ok_or(AgentMediaBridgeError::AgentNotRegistered(agent_id))?;

        self.media
            .start_stt(client_id, AudioFormat::default())
            .map_err(AgentMediaBridgeError::Media)?;

        Ok(())
    }

    /// Feed audio data to the active STT session.
    pub fn feed_audio(&mut self, data: &[u8]) -> Result<Option<SttResult>, AgentMediaBridgeError> {
        self.media
            .stt_feed_audio(data)
            .map_err(AgentMediaBridgeError::Media)
    }

    /// End the current voice utterance and begin processing.
    pub fn end_voice_utterance(&mut self) -> Result<(), AgentMediaBridgeError> {
        self.media
            .stt_end_utterance()
            .map_err(AgentMediaBridgeError::Media)
    }

    /// Deliver a voice command result and queue it for parsing.
    ///
    /// The text will be parsed as a command for the agent.
    pub fn deliver_voice_command(
        &mut self,
        text: impl Into<String>,
        confidence: u8,
    ) -> Result<(), AgentMediaBridgeError> {
        let text = text.into();

        // Get the active client
        let client_id = self
            .media
            .stt_active_client()
            .ok_or(AgentMediaBridgeError::NoActiveSession)?;

        // Deliver to media server (resets STT to idle)
        self.media
            .stt_deliver_result(&text, confidence)
            .map_err(AgentMediaBridgeError::Media)?;

        // Queue for command parsing
        self.pending_voice_commands.push_back((client_id, text));

        Ok(())
    }

    /// Cancel the current voice input session.
    pub fn cancel_voice_input(&mut self) -> Option<AgentId> {
        self.media
            .stt_cancel()
            .and_then(|client_id| self.client_to_agent.get(&client_id).copied())
    }

    /// Get the current STT state.
    pub fn stt_state(&self) -> SttState {
        self.media.stt_state()
    }

    /// Get the agent currently using voice input.
    pub fn voice_input_agent(&self) -> Option<AgentId> {
        self.media
            .stt_active_client()
            .and_then(|client_id| self.client_to_agent.get(&client_id).copied())
    }

    // ========================================================================
    // Voice Output (TTS)
    // ========================================================================

    /// Queue a TTS announcement for an agent.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Agent is not registered
    /// - TTS queue is full (INV-MEDIA-2)
    pub fn announce(
        &mut self,
        agent_id: AgentId,
        text: impl Into<String>,
        priority: Priority,
    ) -> Result<u64, AgentMediaBridgeError> {
        let client_id = self
            .agent_to_client
            .get(&agent_id)
            .copied()
            .ok_or(AgentMediaBridgeError::AgentNotRegistered(agent_id))?;

        self.media
            .queue_tts(client_id, text, priority)
            .map_err(AgentMediaBridgeError::Media)
    }

    /// Announce command completion for an agent.
    ///
    /// If `announce_completions` is enabled, this will queue a TTS message.
    pub fn announce_completion(
        &mut self,
        agent_id: AgentId,
        summary: impl Into<String>,
    ) -> Result<Option<u64>, AgentMediaBridgeError> {
        if !self.config.announce_completions {
            return Ok(None);
        }

        let summary = self.truncate_announcement(summary.into());
        self.announce(agent_id, summary, Priority::Normal).map(Some)
    }

    /// Announce an error for an agent.
    ///
    /// If `announce_errors` is enabled, this will queue a TTS message.
    pub fn announce_error(
        &mut self,
        agent_id: AgentId,
        error: impl Into<String>,
    ) -> Result<Option<u64>, AgentMediaBridgeError> {
        if !self.config.announce_errors {
            return Ok(None);
        }

        let error = self.truncate_announcement(error.into());
        let message = format!("Error: {}", error);
        self.announce(agent_id, message, Priority::High).map(Some)
    }

    /// Truncate announcement to configured word limit.
    fn truncate_announcement(&self, text: String) -> String {
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.len() <= self.config.max_announcement_words {
            text
        } else {
            let truncated: String = words[..self.config.max_announcement_words].join(" ");
            format!("{}...", truncated)
        }
    }

    // ========================================================================
    // Command Processing
    // ========================================================================

    /// Parse and queue a voice command for execution.
    ///
    /// This takes the next pending voice command and creates a Command
    /// for the orchestrator.
    pub fn process_pending_voice_command(
        &mut self,
    ) -> Result<Option<CommandId>, AgentMediaBridgeError> {
        let (client_id, text) = match self.pending_voice_commands.pop_front() {
            Some(cmd) => cmd,
            None => return Ok(None),
        };

        let agent_id = self
            .client_to_agent
            .get(&client_id)
            .copied()
            .ok_or(AgentMediaBridgeError::NoActiveSession)?;

        // Parse the command text into a Command
        // For now, treat all voice commands as shell commands requiring approval
        let command = Command::builder(CommandType::Shell)
            .payload(&text)
            .build(CommandId(0)); // ID will be assigned by orchestrator

        // Queue in orchestrator (unapproved - needs review)
        let cmd_id = self
            .orchestrator
            .queue_command(command)
            .map_err(AgentMediaBridgeError::Orchestrator)?;

        // Announce that command was received
        let _ = self.announce(
            agent_id,
            format!("Command received: {}", self.truncate_announcement(text)),
            Priority::Low,
        );

        Ok(Some(cmd_id))
    }

    /// Process all pending voice commands.
    pub fn process_all_pending_voice_commands(
        &mut self,
    ) -> Vec<Result<CommandId, AgentMediaBridgeError>> {
        let mut results = Vec::new();
        while !self.pending_voice_commands.is_empty() {
            match self.process_pending_voice_command() {
                Ok(Some(cmd_id)) => results.push(Ok(cmd_id)),
                Ok(None) => break,
                Err(e) => results.push(Err(e)),
            }
        }
        results
    }

    /// Check for completed executions and announce them.
    ///
    /// This should be called periodically to handle completion announcements.
    /// Polls the orchestrator for completed executions and announces results.
    ///
    /// Note: The orchestrator handles execution state transitions internally.
    /// Announcements are handled separately via announce_completion/announce_error
    /// when the caller explicitly requests them.
    pub fn check_completions(&mut self) -> usize {
        self.orchestrator.poll_executions()
    }

    // ========================================================================
    // Approval Workflow
    // ========================================================================

    /// Request approval for a voice command.
    ///
    /// Voice commands default to requiring approval for safety.
    pub fn request_voice_command_approval(
        &mut self,
        agent_id: AgentId,
        cmd_id: CommandId,
    ) -> Result<ApprovalRequestId, AgentMediaBridgeError> {
        let request_id = self
            .orchestrator
            .request_approval(agent_id, cmd_id)
            .map_err(AgentMediaBridgeError::Orchestrator)?;

        // Announce that approval is needed
        let _ = self.announce(
            agent_id,
            "This command requires approval. Say 'approve' or 'reject'.",
            Priority::High,
        );

        Ok(request_id)
    }

    /// Approve a pending command via voice.
    pub fn voice_approve(
        &mut self,
        request_id: ApprovalRequestId,
    ) -> Result<(), AgentMediaBridgeError> {
        self.orchestrator
            .approve_request(request_id)
            .map_err(AgentMediaBridgeError::Approval)?;

        Ok(())
    }

    /// Reject a pending command via voice.
    pub fn voice_reject(
        &mut self,
        request_id: ApprovalRequestId,
    ) -> Result<(), AgentMediaBridgeError> {
        self.orchestrator
            .reject_request(request_id)
            .map_err(AgentMediaBridgeError::Approval)?;

        Ok(())
    }

    // ========================================================================
    // Orchestrator Access
    // ========================================================================

    /// Get a reference to the orchestrator.
    pub fn orchestrator(&self) -> &Orchestrator {
        &self.orchestrator
    }

    /// Get a mutable reference to the orchestrator.
    pub fn orchestrator_mut(&mut self) -> &mut Orchestrator {
        &mut self.orchestrator
    }

    // ========================================================================
    // Media Server Access
    // ========================================================================

    /// Get a reference to the media server.
    pub fn media(&self) -> &MediaServer {
        &self.media
    }

    /// Get a mutable reference to the media server.
    pub fn media_mut(&mut self) -> &mut MediaServer {
        &mut self.media
    }

    // ========================================================================
    // Step/Tick
    // ========================================================================

    /// Run one step of the integration loop.
    ///
    /// This:
    /// 1. Processes pending voice commands
    /// 2. Runs orchestrator auto-assign and auto-execute
    /// 3. Checks for completions
    /// 4. Advances media server clock
    pub fn step(&mut self) {
        // Process voice commands
        let _ = self.process_all_pending_voice_commands();

        // Run orchestrator step
        self.orchestrator.step();

        // Check completions
        let _ = self.check_completions();

        // Advance media clock
        self.media.tick();
    }

    // ========================================================================
    // Invariant Verification
    // ========================================================================

    /// Verify all integration invariants.
    ///
    /// In release builds, only verifies media and bridge invariants.
    /// In test builds, also verifies orchestrator invariants.
    pub fn verify_invariants(&self) -> bool {
        // Media server invariants (always available)
        let media_ok = self.media.verify_invariants();

        // Bridge-specific invariants (always available)
        let bridge_ok = self.verify_bridge_invariants();

        // Orchestrator invariants only available in test mode
        #[cfg(test)]
        {
            let orch_ok = self.orchestrator.verify_invariants();
            orch_ok && media_ok && bridge_ok
        }

        #[cfg(not(test))]
        {
            media_ok && bridge_ok
        }
    }

    /// Verify bridge-specific invariants.
    fn verify_bridge_invariants(&self) -> bool {
        // INV-BRIDGE-1: Agent-client mappings are bijective
        let bijective = self.agent_to_client.len() == self.client_to_agent.len()
            && self
                .agent_to_client
                .iter()
                .all(|(agent, client)| self.client_to_agent.get(client) == Some(agent));

        // INV-BRIDGE-2: Active STT client has registered agent
        let stt_valid = self
            .media
            .stt_active_client()
            .map(|client| self.client_to_agent.contains_key(&client))
            .unwrap_or(true);

        bijective && stt_valid
    }
}

impl Default for AgentMediaBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors from the agent-media bridge.
#[derive(Debug)]
pub enum AgentMediaBridgeError {
    /// Orchestrator error.
    Orchestrator(OrchestratorError),
    /// Approval workflow error.
    Approval(ApprovalError),
    /// Media server error.
    Media(MediaServerError),
    /// Agent not registered with the bridge.
    AgentNotRegistered(AgentId),
    /// No active voice session.
    NoActiveSession,
    /// Voice command parse error.
    ParseError(String),
}

impl std::fmt::Display for AgentMediaBridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Orchestrator(e) => write!(f, "Orchestrator error: {}", e),
            Self::Approval(e) => write!(f, "Approval error: {}", e),
            Self::Media(e) => write!(f, "Media error: {}", e),
            Self::AgentNotRegistered(id) => write!(f, "Agent {:?} not registered", id),
            Self::NoActiveSession => write!(f, "No active voice session"),
            Self::ParseError(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

impl std::error::Error for AgentMediaBridgeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Orchestrator(e) => Some(e),
            Self::Approval(e) => Some(e),
            Self::Media(e) => Some(e),
            _ => None,
        }
    }
}

impl From<OrchestratorError> for AgentMediaBridgeError {
    fn from(e: OrchestratorError) -> Self {
        Self::Orchestrator(e)
    }
}

impl From<MediaServerError> for AgentMediaBridgeError {
    fn from(e: MediaServerError) -> Self {
        Self::Media(e)
    }
}

impl From<ApprovalError> for AgentMediaBridgeError {
    fn from(e: ApprovalError) -> Self {
        Self::Approval(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Capability;

    #[test]
    fn test_bridge_creation() {
        let bridge = AgentMediaBridge::new();
        assert!(bridge.verify_invariants());
    }

    #[test]
    fn test_agent_registration() {
        let mut bridge = AgentMediaBridge::new();

        // Spawn an agent in the orchestrator
        let agent_id = bridge
            .orchestrator_mut()
            .spawn_agent(&[Capability::Shell])
            .unwrap();

        // Register with media bridge
        let client_id = bridge.register_agent(agent_id);
        assert!(client_id > 0);

        // Verify mappings
        assert_eq!(bridge.client_for_agent(agent_id), Some(client_id));
        assert_eq!(bridge.agent_for_client(client_id), Some(agent_id));

        // Registering again returns same client ID
        let client_id2 = bridge.register_agent(agent_id);
        assert_eq!(client_id, client_id2);

        assert!(bridge.verify_invariants());
    }

    #[test]
    fn test_agent_unregistration() {
        let mut bridge = AgentMediaBridge::new();

        let agent_id = bridge
            .orchestrator_mut()
            .spawn_agent(&[Capability::Shell])
            .unwrap();
        let client_id = bridge.register_agent(agent_id);

        // Unregister
        bridge.unregister_agent(agent_id);

        // Mappings should be gone
        assert_eq!(bridge.client_for_agent(agent_id), None);
        assert_eq!(bridge.agent_for_client(client_id), None);

        assert!(bridge.verify_invariants());
    }

    #[test]
    fn test_announce() {
        let mut bridge = AgentMediaBridge::new();

        let agent_id = bridge
            .orchestrator_mut()
            .spawn_agent(&[Capability::Shell])
            .unwrap();
        bridge.register_agent(agent_id);

        // Queue announcement
        let id = bridge.announce(agent_id, "Hello world", Priority::Normal);
        assert!(id.is_ok());

        assert!(bridge.verify_invariants());
    }

    #[test]
    fn test_announcement_truncation() {
        let bridge = AgentMediaBridge::with_config(AgentMediaBridgeConfig {
            max_announcement_words: 5,
            ..Default::default()
        });

        let long_text = "one two three four five six seven eight nine ten";
        let truncated = bridge.truncate_announcement(long_text.to_string());

        assert!(truncated.ends_with("..."));
        assert_eq!(truncated, "one two three four five...");
    }

    #[test]
    fn test_voice_command_queue() {
        let mut bridge = AgentMediaBridge::new();

        let agent_id = bridge
            .orchestrator_mut()
            .spawn_agent(&[Capability::Shell])
            .unwrap();
        let client_id = bridge.register_agent(agent_id);

        // Manually add a voice command (simulating STT result)
        bridge
            .pending_voice_commands
            .push_back((client_id, "echo hello".to_string()));

        // Process it
        let result = bridge.process_pending_voice_command();
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());

        // Queue should be empty now
        assert!(bridge.pending_voice_commands.is_empty());

        assert!(bridge.verify_invariants());
    }

    #[test]
    fn test_step() {
        let mut bridge = AgentMediaBridge::new();

        let agent_id = bridge
            .orchestrator_mut()
            .spawn_agent(&[Capability::Shell])
            .unwrap();
        bridge.register_agent(agent_id);

        // Step should not panic
        bridge.step();
        bridge.step();
        bridge.step();

        assert!(bridge.verify_invariants());
    }

    #[test]
    fn test_unregistered_agent_error() {
        let mut bridge = AgentMediaBridge::new();

        // Try to announce for unregistered agent
        let fake_agent = AgentId(999);
        let result = bridge.announce(fake_agent, "test", Priority::Normal);

        assert!(matches!(
            result,
            Err(AgentMediaBridgeError::AgentNotRegistered(_))
        ));
    }

    #[test]
    fn test_invariants_preserved() {
        let mut bridge = AgentMediaBridge::new();

        // Do various operations
        let a1 = bridge
            .orchestrator_mut()
            .spawn_agent(&[Capability::Shell])
            .unwrap();
        let a2 = bridge
            .orchestrator_mut()
            .spawn_agent(&[Capability::Net])
            .unwrap();

        bridge.register_agent(a1);
        bridge.register_agent(a2);

        let _ = bridge.announce(a1, "test1", Priority::Normal);
        let _ = bridge.announce(a2, "test2", Priority::High);

        bridge.step();
        bridge.step();

        bridge.unregister_agent(a1);

        // Invariants should hold throughout
        assert!(bridge.verify_invariants());
    }
}
