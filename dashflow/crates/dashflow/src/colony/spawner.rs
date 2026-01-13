// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Process spawning and worker lifecycle management.
//!
//! This module provides the infrastructure for spawning new DashFlow instances
//! as worker processes, managing their lifecycle, and tracking their state.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;
use uuid::Uuid;

use super::config::{ColonyConfig, SpawnConfig, SpawnTemplate, Task};
use super::system::{SystemMonitor, SystemMonitorError};
use super::topology::DeploymentOption;

/// State of a spawned worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state")]
pub enum WorkerState {
    /// Worker is planned but not yet spawned
    Planned {
        /// The spawn configuration (boxed to reduce enum size)
        config: Box<SpawnConfig>,
        /// When the worker was planned
        planned_at: chrono::DateTime<chrono::Utc>,
    },

    /// Worker is currently being spawned
    Spawning {
        /// When spawning started
        started_at: chrono::DateTime<chrono::Utc>,
    },

    /// Worker is running
    Running {
        /// Process ID
        pid: u32,
        /// Network ID (UUID for colony coordination)
        network_id: Uuid,
        /// When the worker started running
        started_at: chrono::DateTime<chrono::Utc>,
        /// Task progress (0.0 - 1.0)
        progress: f32,
    },

    /// Worker is finishing its task
    Finishing {
        /// Process ID
        pid: u32,
        /// Task result (if available)
        result: Option<TaskResult>,
        /// When finishing started
        started_at: chrono::DateTime<chrono::Utc>,
    },

    /// Worker has terminated
    Terminated {
        /// Exit code
        exit_code: i32,
        /// Total duration
        duration: Duration,
        /// Task result
        result: Option<TaskResult>,
        /// When terminated
        terminated_at: chrono::DateTime<chrono::Utc>,
    },

    /// Worker failed to spawn or crashed
    Failed {
        /// Error message
        error: String,
        /// When the failure occurred
        failed_at: chrono::DateTime<chrono::Utc>,
    },
}

impl WorkerState {
    /// Check if worker is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Terminated { .. } | Self::Failed { .. })
    }

    /// Check if worker is active (spawning or running).
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Spawning { .. } | Self::Running { .. } | Self::Finishing { .. }
        )
    }

    /// Get the process ID if available.
    pub fn pid(&self) -> Option<u32> {
        match self {
            Self::Running { pid, .. } | Self::Finishing { pid, .. } => Some(*pid),
            _ => None,
        }
    }

    /// Get the state name for display.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Planned { .. } => "planned",
            Self::Spawning { .. } => "spawning",
            Self::Running { .. } => "running",
            Self::Finishing { .. } => "finishing",
            Self::Terminated { .. } => "terminated",
            Self::Failed { .. } => "failed",
        }
    }
}

/// Result of a completed task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Whether the task succeeded
    pub success: bool,

    /// Exit code of the process
    pub exit_code: i32,

    /// Task output (stdout)
    pub stdout: String,

    /// Task error output (stderr)
    pub stderr: String,

    /// Structured result data (task-specific)
    pub data: Option<serde_json::Value>,

    /// Duration of task execution
    pub duration: Duration,
}

impl TaskResult {
    /// Create a successful result.
    pub fn success(exit_code: i32, stdout: String, stderr: String, duration: Duration) -> Self {
        Self {
            success: exit_code == 0,
            exit_code,
            stdout,
            stderr,
            data: None,
            duration,
        }
    }

    /// Create a failed result.
    pub fn failure(error: impl Into<String>, duration: Duration) -> Self {
        Self {
            success: false,
            exit_code: -1,
            stdout: String::new(),
            stderr: error.into(),
            data: None,
            duration,
        }
    }

    /// Builder: set structured data.
    #[must_use]
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }
}

/// A spawned worker instance.
#[derive(Debug)]
pub struct Worker {
    /// Unique worker ID
    pub id: Uuid,

    /// Worker name (optional)
    pub name: Option<String>,

    /// Spawn configuration
    pub config: SpawnConfig,

    /// Current state
    pub(crate) state: WorkerState,

    /// Process handle (when spawning/running)
    pub(crate) process: Option<Child>,

    /// Parent worker ID (if spawned by another worker)
    pub parent_id: Option<Uuid>,

    /// Spawn time
    pub created_at: Instant,

    /// Tags
    pub tags: Vec<String>,
}

impl Worker {
    /// Create a new worker in the Planned state.
    pub fn new(config: SpawnConfig) -> Self {
        let name = config.name.clone();
        let tags = config.tags.clone();

        Self {
            id: Uuid::new_v4(),
            name,
            config: config.clone(),
            state: WorkerState::Planned {
                config: Box::new(config),
                planned_at: chrono::Utc::now(),
            },
            process: None,
            parent_id: None,
            created_at: Instant::now(),
            tags,
        }
    }

    /// Create a new worker with a parent ID.
    #[must_use]
    pub fn with_parent(config: SpawnConfig, parent_id: Uuid) -> Self {
        let mut worker = Self::new(config);
        worker.parent_id = Some(parent_id);
        worker
    }

    /// Get the current state.
    pub fn state(&self) -> &WorkerState {
        &self.state
    }

    /// Get the process ID if running.
    pub fn pid(&self) -> Option<u32> {
        self.state.pid()
    }

    /// Check if worker is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        self.state.is_terminal()
    }

    /// Check if worker is active.
    pub fn is_active(&self) -> bool {
        self.state.is_active()
    }

    /// Get time since creation.
    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }

    /// Transition to Spawning state.
    fn transition_to_spawning(&mut self) {
        self.state = WorkerState::Spawning {
            started_at: chrono::Utc::now(),
        };
    }

    /// Transition to Running state.
    fn transition_to_running(&mut self, pid: u32, network_id: Uuid) {
        self.state = WorkerState::Running {
            pid,
            network_id,
            started_at: chrono::Utc::now(),
            progress: 0.0,
        };
    }

    /// Update progress (only valid in Running state).
    pub fn update_progress(&mut self, progress: f32) {
        if let WorkerState::Running { progress: p, .. } = &mut self.state {
            *p = progress.clamp(0.0, 1.0);
        }
    }

    /// Transition to Finishing state.
    #[allow(dead_code)] // Architectural: Reserved for network integration
    fn transition_to_finishing(&mut self, result: Option<TaskResult>) {
        if let WorkerState::Running { pid, .. } = self.state {
            self.state = WorkerState::Finishing {
                pid,
                result,
                started_at: chrono::Utc::now(),
            };
        }
    }

    /// Transition to Terminated state.
    pub(crate) fn transition_to_terminated(&mut self, exit_code: i32, result: Option<TaskResult>) {
        self.state = WorkerState::Terminated {
            exit_code,
            duration: self.created_at.elapsed(),
            result,
            terminated_at: chrono::Utc::now(),
        };
    }

    /// Transition to Failed state.
    pub(crate) fn transition_to_failed(&mut self, error: impl Into<String>) {
        self.state = WorkerState::Failed {
            error: error.into(),
            failed_at: chrono::Utc::now(),
        };
    }

    /// Get mutable access to the process handle.
    #[cfg(feature = "network")]
    pub(crate) fn process_mut(&mut self) -> Option<&mut Child> {
        self.process.as_mut()
    }
}

/// Manages spawning and lifecycle of workers.
#[derive(Debug)]
pub struct Spawner {
    /// Colony configuration
    config: ColonyConfig,

    /// System monitor for resource checking
    monitor: Arc<SystemMonitor>,

    /// Path to the current executable
    self_executable: PathBuf,

    /// Environment variables to inherit
    inherited_env: HashMap<String, String>,
}

impl Spawner {
    /// Create a new spawner.
    pub fn new(config: ColonyConfig, monitor: Arc<SystemMonitor>) -> Result<Self, SpawnError> {
        let self_executable = std::env::current_exe().map_err(|e| {
            SpawnError::ConfigurationError(format!("Failed to get current executable: {}", e))
        })?;

        // Collect environment variables to inherit
        let inherited_env: HashMap<String, String> = std::env::vars()
            .filter(|(k, _)| {
                // Inherit common important env vars
                k.starts_with("RUST_")
                    || k.starts_with("CARGO_")
                    || k.starts_with("PATH")
                    || k.starts_with("HOME")
                    || k.starts_with("USER")
                    || k.starts_with("DASHFLOW_")
                    || k.starts_with("AWS_")
                    || k.starts_with("ANTHROPIC_")
                    || k.starts_with("OPENAI_")
            })
            .collect();

        Ok(Self {
            config,
            monitor,
            self_executable,
            inherited_env,
        })
    }

    /// Spawn a new worker.
    pub async fn spawn(&self, config: SpawnConfig) -> Result<Worker, SpawnError> {
        // Check if spawning is allowed
        if matches!(self.config.approval, super::config::SpawnApproval::Disabled) {
            return Err(SpawnError::SpawnDisabled);
        }

        // Check template allowlist
        let template_name = config.template.name();
        if !self.config.is_template_allowed(template_name) {
            return Err(SpawnError::TemplateNotAllowed(template_name.to_string()));
        }

        // Check resources
        let requirements = config.requirements();
        let can_spawn =
            self.monitor.can_spawn(requirements).await.map_err(|e| {
                SpawnError::ResourceError(format!("Failed to check resources: {}", e))
            })?;

        if !can_spawn {
            return Err(SpawnError::InsufficientResources);
        }

        // Create worker
        let mut worker = Worker::new(config.clone());

        // Spawn based on deployment option
        match config.deployment {
            DeploymentOption::Local | DeploymentOption::Any => {
                self.spawn_process(&mut worker).await?;
            }
            DeploymentOption::Isolated => {
                // Check if Docker is available, otherwise fall back to process
                let topology = self.monitor.topology().await.map_err(|e| {
                    SpawnError::ResourceError(format!("Failed to get topology: {}", e))
                })?;

                if topology.container_runtime.is_some() {
                    self.spawn_docker(&mut worker).await?;
                } else {
                    self.spawn_process(&mut worker).await?;
                }
            }
            DeploymentOption::Distributed => {
                // Kubernetes deployment: Deferred
                // Requires: k8s client, pod spec generation, service mesh integration
                // Current focus: Local and Docker deployments for development
                return Err(SpawnError::UnsupportedDeployment(
                    "Kubernetes deployment deferred - use Local or Docker for now".to_string(),
                ));
            }
        }

        Ok(worker)
    }

    /// Spawn worker as a process.
    async fn spawn_process(&self, worker: &mut Worker) -> Result<(), SpawnError> {
        worker.transition_to_spawning();

        // Build command
        let (executable, args) = self.build_command(&worker.config)?;

        // Build environment
        let mut env = self.inherited_env.clone();
        env.extend(worker.config.environment.clone());

        // Add worker-specific environment
        env.insert("DASHFLOW_WORKER_ID".to_string(), worker.id.to_string());
        env.insert("DASHFLOW_WORKER_MODE".to_string(), "true".to_string());

        if let Some(ref parent_id) = worker.parent_id {
            env.insert("DASHFLOW_PARENT_ID".to_string(), parent_id.to_string());
        }

        // Serialize task to environment
        if let Ok(task_json) = serde_json::to_string(&worker.config.task) {
            env.insert("DASHFLOW_TASK".to_string(), task_json);
        }

        // Create command
        let mut cmd = Command::new(&executable);
        cmd.args(&args)
            .envs(env)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Set working directory
        if let Some(ref wd) = worker.config.working_directory {
            cmd.current_dir(wd);
        }

        // Spawn process
        let child = cmd
            .spawn()
            .map_err(|e| SpawnError::ProcessError(format!("Failed to spawn process: {}", e)))?;

        let pid = child.id().unwrap_or(0);
        let network_id = Uuid::new_v4(); // Would be assigned by network module in full implementation

        worker.process = Some(child);
        worker.transition_to_running(pid, network_id);

        Ok(())
    }

    /// Spawn worker in Docker container.
    async fn spawn_docker(&self, worker: &mut Worker) -> Result<(), SpawnError> {
        worker.transition_to_spawning();

        // Get Docker config from custom app config or use defaults
        let docker_config = match &worker.config.template {
            SpawnTemplate::Custom(app_config) => app_config.docker.clone(),
            _ => None,
        };

        let image = docker_config
            .as_ref()
            .map(|d| d.image_ref())
            .unwrap_or_else(|| "dashflow:latest".to_string());

        // Build docker run command
        let mut args = vec!["run".to_string(), "--rm".to_string()];

        // Add resource limits
        if let Some(max_cpu) = worker.config.resources.max_cpu_cores {
            args.push(format!("--cpus={}", max_cpu));
        }
        if let Some(max_mem) = worker.config.resources.max_memory_mb {
            args.push(format!("--memory={}m", max_mem));
        }

        // Add environment variables
        for (key, value) in &worker.config.environment {
            args.push("-e".to_string());
            args.push(format!("{}={}", key, value));
        }

        // Worker-specific environment
        args.push("-e".to_string());
        args.push(format!("DASHFLOW_WORKER_ID={}", worker.id));
        args.push("-e".to_string());
        args.push("DASHFLOW_WORKER_MODE=true".to_string());

        // Add volumes from docker config
        if let Some(ref dc) = docker_config {
            for vol in &dc.volumes {
                args.push("-v".to_string());
                let mount_str = if vol.read_only {
                    format!(
                        "{}:{}:ro",
                        vol.host_path.display(),
                        vol.container_path.display()
                    )
                } else {
                    format!(
                        "{}:{}",
                        vol.host_path.display(),
                        vol.container_path.display()
                    )
                };
                args.push(mount_str);
            }

            for port in &dc.ports {
                args.push("-p".to_string());
                args.push(format!("{}:{}", port.host_port, port.container_port));
            }
        }

        // Add image
        args.push(image);

        // Build task command
        let task_args = self.task_to_args(&worker.config.task);
        args.extend(task_args);

        // Create command
        let mut cmd = Command::new("docker");
        cmd.args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Spawn process
        let child = cmd.spawn().map_err(|e| {
            SpawnError::ProcessError(format!("Failed to spawn docker container: {}", e))
        })?;

        let pid = child.id().unwrap_or(0);
        let network_id = Uuid::new_v4();

        worker.process = Some(child);
        worker.transition_to_running(pid, network_id);

        Ok(())
    }

    /// Build command and args for a spawn config.
    fn build_command(&self, config: &SpawnConfig) -> Result<(PathBuf, Vec<String>), SpawnError> {
        match &config.template {
            SpawnTemplate::Self_ => {
                // Clone current executable with task args
                let args = self.task_to_args(&config.task);
                Ok((self.self_executable.clone(), args))
            }
            SpawnTemplate::Named(name) => {
                // Use template - for now just use self with template name as arg
                let mut args = vec!["--template".to_string(), name.clone()];
                args.extend(self.task_to_args(&config.task));
                Ok((self.self_executable.clone(), args))
            }
            SpawnTemplate::Custom(app_config) => {
                // Use custom executable
                let mut args = app_config.args.clone();
                args.extend(self.task_to_args(&config.task));
                Ok((app_config.executable.clone(), args))
            }
        }
    }

    /// Convert task to command line arguments.
    fn task_to_args(&self, task: &Task) -> Vec<String> {
        match task {
            Task::Idle => vec!["--idle".to_string()],
            Task::RunTests {
                crate_name,
                test_filter,
            } => {
                let mut args = vec!["test".to_string(), "-p".to_string(), crate_name.clone()];
                if let Some(filter) = test_filter {
                    args.push("--".to_string());
                    args.push(filter.clone());
                }
                args
            }
            Task::Build { target, release } => {
                let mut args = vec!["build".to_string()];
                if *release {
                    args.push("--release".to_string());
                }
                args.push("--target".to_string());
                args.push(target.clone());
                args
            }
            Task::Optimize { target, iterations } => {
                vec![
                    "optimize".to_string(),
                    "--target".to_string(),
                    target.clone(),
                    "--iterations".to_string(),
                    iterations.to_string(),
                ]
            }
            Task::Analyze {
                path,
                analysis_type,
            } => {
                vec![
                    "analyze".to_string(),
                    "--path".to_string(),
                    path.to_string_lossy().to_string(),
                    "--type".to_string(),
                    format!("{:?}", analysis_type).to_lowercase(),
                ]
            }
            Task::Command { command, args } => {
                let mut result = vec!["exec".to_string(), "--".to_string(), command.clone()];
                result.extend(args.clone());
                result
            }
            Task::Custom(data) => {
                vec!["custom".to_string(), "--data".to_string(), data.to_string()]
            }
        }
    }
}

/// Manages multiple workers.
#[derive(Debug)]
pub struct WorkerManager {
    /// All workers (by ID)
    workers: Arc<RwLock<HashMap<Uuid, Worker>>>,

    /// Spawner for creating new workers
    spawner: Arc<Spawner>,

    /// Maximum concurrent workers
    max_workers: u32,
}

impl WorkerManager {
    /// Create a new worker manager.
    pub fn new(spawner: Arc<Spawner>, max_workers: u32) -> Self {
        Self {
            workers: Arc::new(RwLock::new(HashMap::new())),
            spawner,
            max_workers,
        }
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
            workers.insert(id, worker);
        }

        Ok(id)
    }

    /// Get a worker by ID.
    pub async fn get(&self, id: &Uuid) -> Option<WorkerInfo> {
        let workers = self.workers.read().await;
        workers.get(id).map(WorkerInfo::from_worker)
    }

    /// Get all workers.
    pub async fn list(&self) -> Vec<WorkerInfo> {
        let workers = self.workers.read().await;
        workers.values().map(WorkerInfo::from_worker).collect()
    }

    /// Get active workers.
    pub async fn active(&self) -> Vec<WorkerInfo> {
        let workers = self.workers.read().await;
        workers
            .values()
            .filter(|w| w.is_active())
            .map(WorkerInfo::from_worker)
            .collect()
    }

    /// Count active workers.
    pub async fn active_count(&self) -> usize {
        let workers = self.workers.read().await;
        workers.values().filter(|w| w.is_active()).count()
    }

    /// Terminate a worker.
    pub async fn terminate(&self, id: &Uuid) -> Result<(), SpawnError> {
        let mut workers = self.workers.write().await;

        let worker = workers.get_mut(id).ok_or(SpawnError::WorkerNotFound(*id))?;

        if worker.is_terminal() {
            return Ok(()); // Already terminated
        }

        // Kill the process
        if let Some(ref mut child) = worker.process {
            let _ = child.kill().await;
        }

        worker.transition_to_terminated(-1, None);
        Ok(())
    }

    /// Terminate all workers.
    pub async fn terminate_all(&self) -> Vec<Uuid> {
        let mut workers = self.workers.write().await;
        let mut terminated = Vec::new();

        for (id, worker) in workers.iter_mut() {
            if !worker.is_terminal() {
                if let Some(ref mut child) = worker.process {
                    let _ = child.kill().await;
                }
                worker.transition_to_terminated(-1, None);
                terminated.push(*id);
            }
        }

        terminated
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

            {
                let workers = self.workers.read().await;
                let worker = workers.get(id).ok_or(SpawnError::WorkerNotFound(*id))?;

                match &worker.state {
                    WorkerState::Terminated { result, .. } => {
                        return result.clone().ok_or(SpawnError::NoResult);
                    }
                    WorkerState::Failed { error, .. } => {
                        return Err(SpawnError::WorkerFailed(error.clone()));
                    }
                    _ => {}
                }
            }

            // Poll interval
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Check if process has exited
            self.check_process_status(id).await?;
        }
    }

    /// Check and update worker process status.
    async fn check_process_status(&self, id: &Uuid) -> Result<(), SpawnError> {
        let mut workers = self.workers.write().await;
        let worker = workers.get_mut(id).ok_or(SpawnError::WorkerNotFound(*id))?;

        if let Some(ref mut child) = worker.process {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let exit_code = status.code().unwrap_or(-1);
                    let result = TaskResult::success(
                        exit_code,
                        String::new(), // Would capture actual output in production
                        String::new(),
                        worker.created_at.elapsed(),
                    );
                    worker.transition_to_terminated(exit_code, Some(result));
                }
                Ok(None) => {
                    // Still running
                }
                Err(e) => {
                    worker.transition_to_failed(format!("Process check failed: {}", e));
                }
            }
        }

        Ok(())
    }

    /// Clean up terminated workers older than the given duration.
    pub async fn cleanup(&self, max_age: Duration) {
        let mut workers = self.workers.write().await;

        let to_remove: Vec<Uuid> = workers
            .iter()
            .filter(|(_, w)| w.is_terminal() && w.age() > max_age)
            .map(|(id, _)| *id)
            .collect();

        for id in to_remove {
            workers.remove(&id);
        }
    }

    /// Update progress for a worker.
    pub async fn update_progress(&self, id: &Uuid, progress: f32) -> Result<(), SpawnError> {
        let mut workers = self.workers.write().await;
        let worker = workers.get_mut(id).ok_or(SpawnError::WorkerNotFound(*id))?;
        worker.update_progress(progress);
        Ok(())
    }
}

/// Summary information about a worker (for external use).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerInfo {
    /// Worker ID
    pub id: Uuid,

    /// Worker name
    pub name: Option<String>,

    /// Current state name
    pub state: String,

    /// Process ID (if running)
    pub pid: Option<u32>,

    /// Task type
    pub task_type: String,

    /// Progress (0.0 - 1.0)
    pub progress: f32,

    /// Age in seconds
    pub age_secs: u64,

    /// Tags
    pub tags: Vec<String>,

    /// Is in terminal state
    pub is_terminal: bool,
}

impl WorkerInfo {
    /// Create WorkerInfo from a Worker.
    fn from_worker(worker: &Worker) -> Self {
        let progress = match &worker.state {
            WorkerState::Running { progress, .. } => *progress,
            WorkerState::Terminated { .. } => 1.0,
            _ => 0.0,
        };

        Self {
            id: worker.id,
            name: worker.name.clone(),
            state: worker.state.name().to_string(),
            pid: worker.pid(),
            task_type: worker.config.task.type_name().to_string(),
            progress,
            age_secs: worker.age().as_secs(),
            tags: worker.tags.clone(),
            is_terminal: worker.is_terminal(),
        }
    }
}

/// Errors from spawning operations.
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum SpawnError {
    /// Spawning is disabled
    #[error("Spawning is disabled")]
    SpawnDisabled,

    /// Template not allowed
    #[error("Template not allowed: {0}")]
    TemplateNotAllowed(String),

    /// Insufficient resources
    #[error("Insufficient resources to spawn worker")]
    InsufficientResources,

    /// Worker limit reached
    #[error("Worker limit reached: {0}")]
    WorkerLimitReached(u32),

    /// Process spawn error
    #[error("Process error: {0}")]
    ProcessError(String),

    /// Resource check error
    #[error("Resource error: {0}")]
    ResourceError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    /// Unsupported deployment option
    #[error("Unsupported deployment: {0}")]
    UnsupportedDeployment(String),

    /// Worker not found
    #[error("Worker not found: {0}")]
    WorkerNotFound(Uuid),

    /// Worker failed
    #[error("Worker failed: {0}")]
    WorkerFailed(String),

    /// No result available
    #[error("No result available")]
    NoResult,

    /// Wait timeout exceeded
    #[error("Wait timeout exceeded after {0:?}")]
    Timeout(Duration),
}

impl From<SystemMonitorError> for SpawnError {
    fn from(e: SystemMonitorError) -> Self {
        SpawnError::ResourceError(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colony::config::SpawnApproval;

    #[test]
    fn test_worker_state_lifecycle() {
        let config = SpawnConfig::default();
        let mut worker = Worker::new(config);

        // Initial state is Planned
        assert!(matches!(worker.state(), WorkerState::Planned { .. }));
        assert!(!worker.is_active());
        assert!(!worker.is_terminal());

        // Transition to Spawning
        worker.transition_to_spawning();
        assert!(matches!(worker.state(), WorkerState::Spawning { .. }));
        assert!(worker.is_active());

        // Transition to Running
        worker.transition_to_running(12345, Uuid::new_v4());
        assert!(matches!(worker.state(), WorkerState::Running { .. }));
        assert_eq!(worker.pid(), Some(12345));
        assert!(worker.is_active());

        // Update progress
        worker.update_progress(0.5);
        if let WorkerState::Running { progress, .. } = worker.state() {
            assert!((progress - 0.5).abs() < 0.01);
        }

        // Transition to Terminated
        worker.transition_to_terminated(0, None);
        assert!(matches!(worker.state(), WorkerState::Terminated { .. }));
        assert!(worker.is_terminal());
        assert!(!worker.is_active());
    }

    #[test]
    fn test_worker_state_failed() {
        let config = SpawnConfig::default();
        let mut worker = Worker::new(config);

        worker.transition_to_failed("Something went wrong");
        assert!(matches!(worker.state(), WorkerState::Failed { .. }));
        assert!(worker.is_terminal());

        if let WorkerState::Failed { error, .. } = worker.state() {
            assert_eq!(error, "Something went wrong");
        }
    }

    #[test]
    fn test_worker_with_parent() {
        let config = SpawnConfig::default();
        let parent_id = Uuid::new_v4();
        let worker = Worker::with_parent(config, parent_id);

        assert_eq!(worker.parent_id, Some(parent_id));
    }

    #[test]
    fn test_task_result_success() {
        let result = TaskResult::success(
            0,
            "output".to_string(),
            "".to_string(),
            Duration::from_secs(10),
        );

        assert!(result.success);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout, "output");
    }

    #[test]
    fn test_task_result_failure() {
        let result = TaskResult::failure("error message", Duration::from_secs(5));

        assert!(!result.success);
        assert_eq!(result.exit_code, -1);
        assert_eq!(result.stderr, "error message");
    }

    #[test]
    fn test_worker_info_from_worker() {
        let config = SpawnConfig::with_task(Task::test("my-crate"))
            .name("test-worker")
            .tag("test");
        let worker = Worker::new(config);

        let info = WorkerInfo::from_worker(&worker);

        assert_eq!(info.name, Some("test-worker".to_string()));
        assert_eq!(info.state, "planned");
        assert_eq!(info.task_type, "run_tests");
        assert!(info.tags.contains(&"test".to_string()));
    }

    #[test]
    fn test_spawn_error_display() {
        let err = SpawnError::WorkerLimitReached(10);
        assert_eq!(err.to_string(), "Worker limit reached: 10");

        let err = SpawnError::TemplateNotAllowed("dangerous".to_string());
        assert_eq!(err.to_string(), "Template not allowed: dangerous");
    }

    #[tokio::test]
    async fn test_spawner_disabled() {
        let config = ColonyConfig {
            approval: SpawnApproval::Disabled,
            ..Default::default()
        };

        let monitor = Arc::new(SystemMonitor::new().unwrap());
        let spawner = Spawner::new(config, monitor).unwrap();

        let spawn_config = SpawnConfig::default();
        let result = spawner.spawn(spawn_config).await;

        assert!(matches!(result, Err(SpawnError::SpawnDisabled)));
    }

    #[tokio::test]
    async fn test_spawner_template_not_allowed() {
        let config = ColonyConfig {
            approval: SpawnApproval::Automatic,
            banned_templates: vec!["dangerous".to_string()],
            ..Default::default()
        };

        let monitor = Arc::new(SystemMonitor::new().unwrap());
        let spawner = Spawner::new(config, monitor).unwrap();

        let spawn_config =
            SpawnConfig::default().template(SpawnTemplate::Named("dangerous".to_string()));
        let result = spawner.spawn(spawn_config).await;

        assert!(matches!(result, Err(SpawnError::TemplateNotAllowed(_))));
    }

    #[tokio::test]
    async fn test_worker_manager_basic() {
        let colony_config = ColonyConfig::default();
        let monitor = Arc::new(SystemMonitor::new().unwrap());
        let spawner = Arc::new(Spawner::new(colony_config, monitor).unwrap());
        let manager = WorkerManager::new(spawner, 10);

        assert_eq!(manager.active_count().await, 0);
        assert!(manager.list().await.is_empty());
    }

    #[test]
    fn test_worker_state_name() {
        assert_eq!(
            WorkerState::Planned {
                config: Box::new(SpawnConfig::default()),
                planned_at: chrono::Utc::now()
            }
            .name(),
            "planned"
        );

        assert_eq!(
            WorkerState::Running {
                pid: 0,
                network_id: Uuid::new_v4(),
                started_at: chrono::Utc::now(),
                progress: 0.0
            }
            .name(),
            "running"
        );

        assert_eq!(
            WorkerState::Terminated {
                exit_code: 0,
                duration: Duration::from_secs(1),
                result: None,
                terminated_at: chrono::Utc::now()
            }
            .name(),
            "terminated"
        );
    }
}
