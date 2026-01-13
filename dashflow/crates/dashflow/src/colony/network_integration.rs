// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Network integration for colony workers.
//!
//! This module provides the bridge between the colony spawning system and the
//! network coordination layer. Workers automatically join the network on spawn
//! and report progress/results back to the parent.
//!
//! ## Architecture
//!
//! ```text
//! Parent App                          Worker Process
//! ┌──────────────────┐               ┌──────────────────┐
//! │ NetworkedWorker  │               │ Worker Executable│
//! │ Manager          │◄─────────────►│ (joins network)  │
//! │                  │  Network      │                  │
//! │ - spawn()        │  Messages     │ - reports status │
//! │ - wait_result()  │               │ - sends result   │
//! │ - get_progress() │               │                  │
//! └──────────────────┘               └──────────────────┘
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::config::Task;
use super::spawner::{TaskResult, Worker, WorkerInfo, WorkerState};
use crate::core::config_loader::env_vars::{
    env_bool, env_string, DASHFLOW_NETWORK_ENABLED, DASHFLOW_PARENT_PEER_ID, DASHFLOW_TASK,
    DASHFLOW_WORKER_ID, DASHFLOW_WORKER_MODE,
};

// PeerId is always available from network types (no feature gate)
use crate::network::PeerId;

#[cfg(feature = "network")]
use std::collections::HashMap;
#[cfg(feature = "network")]
use std::sync::Arc;
#[cfg(feature = "network")]
use std::time::{Duration, Instant};
#[cfg(feature = "network")]
use tokio::sync::RwLock;

#[cfg(feature = "network")]
use crate::constants::DEFAULT_HTTP_REQUEST_TIMEOUT;
#[cfg(feature = "network")]
use super::config::{ColonyConfig, SpawnConfig};
#[cfg(feature = "network")]
use super::spawner::{SpawnError, Spawner};
#[cfg(feature = "network")]
use super::system::SystemMonitor;

#[cfg(feature = "network")]
use crate::network::{DashflowNetwork, NetworkConfig, NetworkError, Priority};

/// Standard channel for worker messages.
pub const WORKER_CHANNEL: &str = "_workers";

/// Worker join channel - used by workers to announce they've started.
pub const WORKER_JOIN_CHANNEL: &str = "_worker_join";

/// Worker result channel - used by workers to send results.
pub const WORKER_RESULT_CHANNEL: &str = "_worker_result";

// ============================================================================
// Worker Messages
// ============================================================================

/// Messages sent between parent and workers over the network.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WorkerMessage {
    /// Worker has joined the network
    WorkerJoined {
        /// Worker ID
        worker_id: Uuid,
        /// Parent ID (who spawned this worker)
        parent_id: Uuid,
        /// Task being executed
        task: Task,
        /// Worker's network peer ID
        peer_id: String,
        /// When the worker started
        started_at: DateTime<Utc>,
    },

    /// Worker progress update
    Progress {
        /// Worker ID
        worker_id: Uuid,
        /// Progress (0.0 - 1.0)
        progress: f32,
        /// Optional status message
        status: Option<String>,
        /// Optional structured data
        data: Option<serde_json::Value>,
    },

    /// Worker task result
    TaskResult {
        /// Worker ID
        worker_id: Uuid,
        /// Whether the task succeeded
        success: bool,
        /// Exit code
        exit_code: i32,
        /// Standard output
        stdout: String,
        /// Standard error
        stderr: String,
        /// Structured result data
        data: Option<serde_json::Value>,
        /// Task duration in milliseconds
        duration_ms: u64,
    },

    /// Worker is leaving the network
    WorkerLeaving {
        /// Worker ID
        worker_id: Uuid,
        /// Reason for leaving
        reason: String,
    },

    /// Worker encountered an error
    WorkerError {
        /// Worker ID
        worker_id: Uuid,
        /// Error message
        error: String,
        /// Whether this is fatal
        fatal: bool,
    },

    /// Request for worker status (from parent)
    StatusRequest {
        /// Worker ID to query
        worker_id: Uuid,
        /// Request ID for correlation
        request_id: Uuid,
    },

    /// Response to status request
    StatusResponse {
        /// Worker ID
        worker_id: Uuid,
        /// Request ID for correlation
        request_id: Uuid,
        /// Current state
        state: String,
        /// Current progress
        progress: f32,
        /// Task type
        task_type: String,
    },
}

impl WorkerMessage {
    /// Create a WorkerJoined message.
    pub fn joined(worker_id: Uuid, parent_id: Uuid, task: Task, peer_id: PeerId) -> Self {
        Self::WorkerJoined {
            worker_id,
            parent_id,
            task,
            peer_id: peer_id.as_uuid().to_string(),
            started_at: Utc::now(),
        }
    }

    /// Create a Progress message.
    pub fn progress(worker_id: Uuid, progress: f32) -> Self {
        Self::Progress {
            worker_id,
            progress,
            status: None,
            data: None,
        }
    }

    /// Create a Progress message with status.
    #[must_use]
    pub fn progress_with_status(worker_id: Uuid, progress: f32, status: impl Into<String>) -> Self {
        Self::Progress {
            worker_id,
            progress,
            status: Some(status.into()),
            data: None,
        }
    }

    /// Create a TaskResult message from a result.
    pub fn task_result(worker_id: Uuid, result: &TaskResult) -> Self {
        Self::TaskResult {
            worker_id,
            success: result.success,
            exit_code: result.exit_code,
            stdout: result.stdout.clone(),
            stderr: result.stderr.clone(),
            data: result.data.clone(),
            duration_ms: result.duration.as_millis() as u64,
        }
    }

    /// Create a WorkerLeaving message.
    pub fn leaving(worker_id: Uuid, reason: impl Into<String>) -> Self {
        Self::WorkerLeaving {
            worker_id,
            reason: reason.into(),
        }
    }

    /// Create a WorkerError message.
    pub fn error(worker_id: Uuid, error: impl Into<String>, fatal: bool) -> Self {
        Self::WorkerError {
            worker_id,
            error: error.into(),
            fatal,
        }
    }

    /// Get the worker ID from this message.
    pub fn worker_id(&self) -> Uuid {
        match self {
            Self::WorkerJoined { worker_id, .. } => *worker_id,
            Self::Progress { worker_id, .. } => *worker_id,
            Self::TaskResult { worker_id, .. } => *worker_id,
            Self::WorkerLeaving { worker_id, .. } => *worker_id,
            Self::WorkerError { worker_id, .. } => *worker_id,
            Self::StatusRequest { worker_id, .. } => *worker_id,
            Self::StatusResponse { worker_id, .. } => *worker_id,
        }
    }

    /// Check if this message is from a specific worker.
    pub fn is_from_worker(&self, id: Uuid) -> bool {
        self.worker_id() == id
    }
}

// ============================================================================
// Networked Worker Info
// ============================================================================

/// Extended worker info that includes network status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkedWorkerInfo {
    /// Basic worker info
    pub info: WorkerInfo,

    /// Network peer ID (if connected)
    pub peer_id: Option<PeerId>,

    /// Whether worker is connected to network
    pub network_connected: bool,

    /// Last message received from worker
    pub last_message_at: Option<DateTime<Utc>>,

    /// Latest status message from worker
    pub status_message: Option<String>,

    /// Parent ID (who spawned this worker)
    pub parent_id: Option<Uuid>,
}

impl NetworkedWorkerInfo {
    /// Create from basic WorkerInfo.
    pub fn from_worker_info(info: WorkerInfo) -> Self {
        Self {
            info,
            peer_id: None,
            network_connected: false,
            last_message_at: None,
            status_message: None,
            parent_id: None,
        }
    }

    /// Update with network peer ID.
    #[must_use]
    pub fn with_peer_id(mut self, peer_id: PeerId) -> Self {
        self.peer_id = Some(peer_id);
        self.network_connected = true;
        self
    }

    /// Update last message time.
    pub fn touch(&mut self) {
        self.last_message_at = Some(Utc::now());
    }
}

// ============================================================================
// Networked Spawner
// ============================================================================

/// A spawner that automatically configures workers to join the network.
#[cfg(feature = "network")]
#[derive(Debug)]
pub struct NetworkedSpawner {
    /// Underlying spawner
    spawner: Arc<Spawner>,

    /// Parent's network identity
    parent_peer_id: PeerId,

    /// Network configuration to pass to workers (reserved for future use)
    #[allow(dead_code)] // Architectural: Reserved for worker network config propagation
    network_config: NetworkConfig,
}

#[cfg(feature = "network")]
impl NetworkedSpawner {
    /// Create a new networked spawner.
    pub fn new(
        colony_config: ColonyConfig,
        monitor: Arc<SystemMonitor>,
        parent_peer_id: PeerId,
        network_config: NetworkConfig,
    ) -> Result<Self, SpawnError> {
        let spawner = Arc::new(Spawner::new(colony_config, monitor)?);
        Ok(Self {
            spawner,
            parent_peer_id,
            network_config,
        })
    }

    /// Create from existing spawner.
    pub fn from_spawner(
        spawner: Arc<Spawner>,
        parent_peer_id: PeerId,
        network_config: NetworkConfig,
    ) -> Self {
        Self {
            spawner,
            parent_peer_id,
            network_config,
        }
    }

    /// Get the parent peer ID.
    pub fn parent_peer_id(&self) -> PeerId {
        self.parent_peer_id
    }

    /// Spawn a worker configured to join the network.
    pub async fn spawn(&self, mut config: SpawnConfig) -> Result<Worker, SpawnError> {
        // Add network-related environment variables
        config.environment.insert(
            "DASHFLOW_PARENT_PEER_ID".to_string(),
            self.parent_peer_id.as_uuid().to_string(),
        );

        config
            .environment
            .insert("DASHFLOW_NETWORK_ENABLED".to_string(), "true".to_string());

        // Pass network port hint (worker will use 0 for auto-assign)
        config
            .environment
            .insert("DASHFLOW_NETWORK_PORT".to_string(), "0".to_string());

        // Spawn the worker
        self.spawner.spawn(config).await
    }

    /// Get the underlying spawner.
    pub fn inner(&self) -> &Arc<Spawner> {
        &self.spawner
    }
}

// ============================================================================
// Networked Worker Manager
// ============================================================================

/// Tracks network state for a worker.
#[cfg(feature = "network")]
#[derive(Debug)]
struct NetworkedWorkerState {
    /// Worker instance
    worker: Worker,
    /// Network peer ID (once connected)
    peer_id: Option<PeerId>,
    /// Last message timestamp
    last_message_at: Option<DateTime<Utc>>,
    /// Latest status message
    status_message: Option<String>,
    /// Received result (before process terminates)
    received_result: Option<TaskResult>,
}

/// Manages workers with network integration.
///
/// Workers automatically join the network and report progress/results.
/// The manager handles message routing and result collection.
#[cfg(feature = "network")]
pub struct NetworkedWorkerManager {
    /// Networked spawner
    spawner: Arc<NetworkedSpawner>,

    /// Network connection
    network: Arc<DashflowNetwork>,

    /// Workers with network state
    workers: Arc<RwLock<HashMap<Uuid, NetworkedWorkerState>>>,

    /// Maximum concurrent workers
    max_workers: u32,

    /// Message receive timeout for wait operations
    message_timeout: Duration,
}

#[cfg(feature = "network")]
impl NetworkedWorkerManager {
    /// Create a new networked worker manager.
    pub fn new(
        spawner: Arc<NetworkedSpawner>,
        network: Arc<DashflowNetwork>,
        max_workers: u32,
    ) -> Self {
        Self {
            spawner,
            network,
            workers: Arc::new(RwLock::new(HashMap::new())),
            max_workers,
            message_timeout: DEFAULT_HTTP_REQUEST_TIMEOUT, // 30s from constants
        }
    }

    /// Set the message timeout.
    #[must_use]
    pub fn with_message_timeout(mut self, timeout: Duration) -> Self {
        self.message_timeout = timeout;
        self
    }

    /// Get the network.
    pub fn network(&self) -> &Arc<DashflowNetwork> {
        &self.network
    }

    /// Subscribe to worker channels.
    pub async fn subscribe_to_worker_channels(&self) {
        self.network.subscribe(WORKER_CHANNEL).await;
        self.network.subscribe(WORKER_JOIN_CHANNEL).await;
        self.network.subscribe(WORKER_RESULT_CHANNEL).await;
    }

    /// Spawn a new worker.
    pub async fn spawn(&self, config: SpawnConfig) -> Result<Uuid, SpawnError> {
        // Check worker limit
        let active_count = self.active_count().await;
        if active_count >= self.max_workers as usize {
            return Err(SpawnError::WorkerLimitReached(self.max_workers));
        }

        // Spawn worker
        let worker = self.spawner.spawn(config).await?;
        let id = worker.id;

        // Register worker
        {
            let mut workers = self.workers.write().await;
            workers.insert(
                id,
                NetworkedWorkerState {
                    worker,
                    peer_id: None,
                    last_message_at: None,
                    status_message: None,
                    received_result: None,
                },
            );
        }

        Ok(id)
    }

    /// Get worker info by ID.
    pub async fn get(&self, id: &Uuid) -> Option<NetworkedWorkerInfo> {
        let workers = self.workers.read().await;
        workers.get(id).map(|state| {
            let mut info = NetworkedWorkerInfo::from_worker_info(WorkerInfo::from(&state.worker));
            info.peer_id = state.peer_id;
            info.network_connected = state.peer_id.is_some();
            info.last_message_at = state.last_message_at;
            info.status_message = state.status_message.clone();
            info.parent_id = state.worker.parent_id;
            info
        })
    }

    /// List all workers.
    pub async fn list(&self) -> Vec<NetworkedWorkerInfo> {
        let workers = self.workers.read().await;
        workers
            .values()
            .map(|state| {
                let mut info =
                    NetworkedWorkerInfo::from_worker_info(WorkerInfo::from(&state.worker));
                info.peer_id = state.peer_id;
                info.network_connected = state.peer_id.is_some();
                info.last_message_at = state.last_message_at;
                info.status_message = state.status_message.clone();
                info
            })
            .collect()
    }

    /// Get active workers.
    pub async fn active(&self) -> Vec<NetworkedWorkerInfo> {
        self.list()
            .await
            .into_iter()
            .filter(|w| !w.info.is_terminal)
            .collect()
    }

    /// Count active workers.
    pub async fn active_count(&self) -> usize {
        let workers = self.workers.read().await;
        workers.values().filter(|w| w.worker.is_active()).count()
    }

    /// Handle an incoming worker message.
    pub async fn handle_message(&self, msg: &WorkerMessage) {
        let worker_id = msg.worker_id();

        let mut workers = self.workers.write().await;
        if let Some(state) = workers.get_mut(&worker_id) {
            state.last_message_at = Some(Utc::now());

            match msg {
                WorkerMessage::WorkerJoined { peer_id, .. } => {
                    if let Ok(uuid) = Uuid::parse_str(peer_id) {
                        state.peer_id = Some(PeerId::from_uuid(uuid));
                    }
                }
                WorkerMessage::Progress {
                    progress, status, ..
                } => {
                    state.worker.update_progress(*progress);
                    if let Some(s) = status {
                        state.status_message = Some(s.clone());
                    }
                }
                WorkerMessage::TaskResult {
                    success,
                    exit_code,
                    stdout,
                    stderr,
                    data,
                    duration_ms,
                    ..
                } => {
                    let result = TaskResult {
                        success: *success,
                        exit_code: *exit_code,
                        stdout: stdout.clone(),
                        stderr: stderr.clone(),
                        data: data.clone(),
                        duration: Duration::from_millis(*duration_ms),
                    };
                    state.received_result = Some(result);
                }
                WorkerMessage::WorkerLeaving { .. } => {
                    state.peer_id = None;
                }
                WorkerMessage::WorkerError { error, fatal, .. } => {
                    state.status_message = Some(format!("Error: {}", error));
                    if *fatal {
                        // Worker will likely terminate soon
                    }
                }
                WorkerMessage::StatusRequest { .. } | WorkerMessage::StatusResponse { .. } => {
                    // These are for worker-side handling
                }
            }
        }
    }

    /// Process incoming network messages and route worker messages.
    pub async fn process_network_messages(&self) {
        while let Some(msg) = self.network.next_message().await {
            if let Ok(worker_msg) = serde_json::from_value::<WorkerMessage>(msg.payload.clone()) {
                self.handle_message(&worker_msg).await;
            }
        }
    }

    /// Default timeout for waiting on workers (1 hour).
    pub const DEFAULT_WAIT_TIMEOUT: Duration = Duration::from_secs(3600);

    /// Wait for a worker to complete with default timeout (1 hour).
    pub async fn wait(&self, id: &Uuid) -> Result<TaskResult, SpawnError> {
        self.wait_with_timeout(id, Self::DEFAULT_WAIT_TIMEOUT).await
    }

    /// Wait for a worker to complete with a specified timeout.
    ///
    /// Returns `SpawnError::Timeout` if the worker does not complete within
    /// the specified duration.
    pub async fn wait_with_timeout(
        &self,
        id: &Uuid,
        timeout: Duration,
    ) -> Result<TaskResult, SpawnError> {
        let start = Instant::now();

        loop {
            // Check timeout
            if start.elapsed() > timeout {
                return Err(SpawnError::Timeout(timeout));
            }

            // Check if we have a result
            {
                let workers = self.workers.read().await;
                let state = workers.get(id).ok_or(SpawnError::WorkerNotFound(*id))?;

                // Check for received network result first
                if let Some(ref result) = state.received_result {
                    return Ok(result.clone());
                }

                // Check worker state
                match &state.worker.state() {
                    WorkerState::Terminated { result, .. } => {
                        return result.clone().ok_or(SpawnError::NoResult);
                    }
                    WorkerState::Failed { error, .. } => {
                        return Err(SpawnError::WorkerFailed(error.clone()));
                    }
                    _ => {}
                }
            }

            // Process network messages
            self.process_network_messages().await;

            // Check process status
            self.check_process_status(id).await?;

            // Small delay to avoid busy loop
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Check and update worker process status.
    async fn check_process_status(&self, id: &Uuid) -> Result<(), SpawnError> {
        let mut workers = self.workers.write().await;
        let state = workers.get_mut(id).ok_or(SpawnError::WorkerNotFound(*id))?;

        if let Some(ref mut child) = state.worker.process_mut() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let exit_code = status.code().unwrap_or(-1);

                    // Prefer network result if we have one
                    let result = state.received_result.take().unwrap_or_else(|| {
                        TaskResult::success(
                            exit_code,
                            String::new(),
                            String::new(),
                            state.worker.age(),
                        )
                    });

                    state
                        .worker
                        .transition_to_terminated(exit_code, Some(result));
                }
                Ok(None) => {
                    // Still running
                }
                Err(e) => {
                    state
                        .worker
                        .transition_to_failed(format!("Process check failed: {}", e));
                }
            }
        }

        Ok(())
    }

    /// Terminate a worker.
    pub async fn terminate(&self, id: &Uuid) -> Result<(), SpawnError> {
        let mut workers = self.workers.write().await;
        let state = workers.get_mut(id).ok_or(SpawnError::WorkerNotFound(*id))?;

        if state.worker.is_terminal() {
            return Ok(());
        }

        // Kill the process
        if let Some(ref mut child) = state.worker.process_mut() {
            let _ = child.kill().await;
        }

        state.worker.transition_to_terminated(-1, None);
        Ok(())
    }

    /// Terminate all workers.
    pub async fn terminate_all(&self) -> Vec<Uuid> {
        let mut workers = self.workers.write().await;
        let mut terminated = Vec::new();

        for (id, state) in workers.iter_mut() {
            if !state.worker.is_terminal() {
                if let Some(ref mut child) = state.worker.process_mut() {
                    let _ = child.kill().await;
                }
                state.worker.transition_to_terminated(-1, None);
                terminated.push(*id);
            }
        }

        terminated
    }

    /// Cleanup old terminated workers.
    pub async fn cleanup(&self, max_age: Duration) {
        let mut workers = self.workers.write().await;

        let to_remove: Vec<Uuid> = workers
            .iter()
            .filter(|(_, state)| state.worker.is_terminal() && state.worker.age() > max_age)
            .map(|(id, _)| *id)
            .collect();

        for id in to_remove {
            workers.remove(&id);
        }
    }

    /// Send a status request to a worker.
    pub async fn request_status(&self, worker_id: Uuid) -> Result<Uuid, NetworkError> {
        let request_id = Uuid::new_v4();
        let msg = WorkerMessage::StatusRequest {
            worker_id,
            request_id,
        };

        self.network
            .broadcast(
                WORKER_CHANNEL,
                serde_json::to_value(&msg).unwrap_or_default(),
                Priority::Normal,
            )
            .await?;

        Ok(request_id)
    }

    /// Broadcast a message to all workers.
    pub async fn broadcast_to_workers(&self, msg: WorkerMessage) -> Result<Uuid, NetworkError> {
        self.network
            .broadcast(
                WORKER_CHANNEL,
                serde_json::to_value(&msg).unwrap_or_default(),
                Priority::Normal,
            )
            .await
    }
}

// ============================================================================
// Worker-side helpers
// ============================================================================

/// Check if this process is running as a worker.
pub fn is_worker_mode() -> bool {
    env_bool(DASHFLOW_WORKER_MODE, false)
}

/// Get the worker ID if running in worker mode.
pub fn worker_id() -> Option<Uuid> {
    env_string(DASHFLOW_WORKER_ID).and_then(|s| Uuid::parse_str(&s).ok())
}

/// Get the parent peer ID if running in worker mode.
pub fn parent_peer_id() -> Option<String> {
    env_string(DASHFLOW_PARENT_PEER_ID)
}

/// Get the task if running in worker mode.
pub fn worker_task() -> Option<Task> {
    env_string(DASHFLOW_TASK).and_then(|s| serde_json::from_str(&s).ok())
}

/// Check if network is enabled for this worker.
pub fn network_enabled() -> bool {
    env_bool(DASHFLOW_NETWORK_ENABLED, false)
}

/// Helper for workers to report progress.
#[cfg(feature = "network")]
pub async fn report_progress(
    network: &DashflowNetwork,
    progress: f32,
    status: Option<&str>,
) -> Result<Uuid, NetworkError> {
    if let Some(id) = worker_id() {
        let msg = if let Some(s) = status {
            WorkerMessage::progress_with_status(id, progress, s)
        } else {
            WorkerMessage::progress(id, progress)
        };

        network
            .broadcast(
                WORKER_CHANNEL,
                serde_json::to_value(&msg).unwrap_or_default(),
                Priority::Background,
            )
            .await
    } else {
        Err(NetworkError::NotStarted)
    }
}

/// Helper for workers to report completion.
#[cfg(feature = "network")]
pub async fn report_result(
    network: &DashflowNetwork,
    result: &TaskResult,
) -> Result<Uuid, NetworkError> {
    if let Some(id) = worker_id() {
        let msg = WorkerMessage::task_result(id, result);
        network
            .broadcast(
                WORKER_RESULT_CHANNEL,
                serde_json::to_value(&msg).unwrap_or_default(),
                Priority::Normal,
            )
            .await
    } else {
        Err(NetworkError::NotStarted)
    }
}

/// Helper for workers to announce they've joined.
#[cfg(feature = "network")]
pub async fn announce_worker_joined(
    network: &DashflowNetwork,
    task: Task,
) -> Result<Uuid, NetworkError> {
    if let (Some(worker_id), Some(parent_id_str)) = (worker_id(), parent_peer_id()) {
        if let Ok(parent_id) = Uuid::parse_str(&parent_id_str) {
            let msg = WorkerMessage::joined(worker_id, parent_id, task, network.peer_id());
            return network
                .broadcast(
                    WORKER_JOIN_CHANNEL,
                    serde_json::to_value(&msg).unwrap_or_default(),
                    Priority::Normal,
                )
                .await;
        }
    }
    Err(NetworkError::NotStarted)
}

// ============================================================================
// WorkerInfo conversion
// ============================================================================

impl From<&Worker> for WorkerInfo {
    fn from(worker: &Worker) -> Self {
        let progress = match worker.state() {
            WorkerState::Running { progress, .. } => *progress,
            WorkerState::Terminated { .. } => 1.0,
            _ => 0.0,
        };

        WorkerInfo {
            id: worker.id,
            name: worker.name.clone(),
            state: worker.state().name().to_string(),
            pid: worker.pid(),
            task_type: worker.config.task.type_name().to_string(),
            progress,
            age_secs: worker.age().as_secs(),
            tags: worker.tags.clone(),
            is_terminal: worker.is_terminal(),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_worker_message_types() {
        let worker_id = Uuid::new_v4();
        let parent_id = Uuid::new_v4();

        // Test WorkerJoined
        let msg = WorkerMessage::joined(worker_id, parent_id, Task::Idle, PeerId::new());
        assert_eq!(msg.worker_id(), worker_id);
        assert!(msg.is_from_worker(worker_id));

        // Test Progress
        let msg = WorkerMessage::progress(worker_id, 0.5);
        if let WorkerMessage::Progress { progress, .. } = msg {
            assert!((progress - 0.5).abs() < 0.01);
        } else {
            panic!("Expected Progress message");
        }

        // Test Progress with status
        let msg = WorkerMessage::progress_with_status(worker_id, 0.75, "Running tests");
        if let WorkerMessage::Progress { status, .. } = msg {
            assert_eq!(status, Some("Running tests".to_string()));
        }

        // Test WorkerError
        let msg = WorkerMessage::error(worker_id, "Something failed", true);
        if let WorkerMessage::WorkerError { error, fatal, .. } = msg {
            assert_eq!(error, "Something failed");
            assert!(fatal);
        }

        // Test WorkerLeaving
        let msg = WorkerMessage::leaving(worker_id, "Task complete");
        if let WorkerMessage::WorkerLeaving { reason, .. } = msg {
            assert_eq!(reason, "Task complete");
        }
    }

    #[test]
    fn test_worker_message_serialization() {
        let worker_id = Uuid::new_v4();
        let msg = WorkerMessage::progress(worker_id, 0.5);

        let json = serde_json::to_string(&msg).unwrap();
        let parsed: WorkerMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.worker_id(), worker_id);
    }

    #[test]
    fn test_task_result_message() {
        let worker_id = Uuid::new_v4();
        let result = TaskResult {
            success: true,
            exit_code: 0,
            stdout: "Test passed".to_string(),
            stderr: String::new(),
            data: Some(serde_json::json!({"tests": 42})),
            duration: Duration::from_secs(10),
        };

        let msg = WorkerMessage::task_result(worker_id, &result);

        if let WorkerMessage::TaskResult {
            success,
            exit_code,
            stdout,
            duration_ms,
            ..
        } = msg
        {
            assert!(success);
            assert_eq!(exit_code, 0);
            assert_eq!(stdout, "Test passed");
            assert_eq!(duration_ms, 10_000);
        } else {
            panic!("Expected TaskResult message");
        }
    }

    #[test]
    fn test_networked_worker_info() {
        let info = WorkerInfo {
            id: Uuid::new_v4(),
            name: Some("test-worker".to_string()),
            state: "running".to_string(),
            pid: Some(12345),
            task_type: "run_tests".to_string(),
            progress: 0.5,
            age_secs: 30,
            tags: vec!["test".to_string()],
            is_terminal: false,
        };

        let networked =
            NetworkedWorkerInfo::from_worker_info(info.clone()).with_peer_id(PeerId::new());

        assert!(networked.network_connected);
        assert!(networked.peer_id.is_some());
        assert_eq!(networked.info.name, Some("test-worker".to_string()));
    }

    #[test]
    fn test_is_worker_mode() {
        // Default should be false
        assert!(!is_worker_mode());
        assert!(worker_id().is_none());
        assert!(parent_peer_id().is_none());
    }

    #[test]
    fn test_channel_constants() {
        assert_eq!(WORKER_CHANNEL, "_workers");
        assert_eq!(WORKER_JOIN_CHANNEL, "_worker_join");
        assert_eq!(WORKER_RESULT_CHANNEL, "_worker_result");
    }
}
