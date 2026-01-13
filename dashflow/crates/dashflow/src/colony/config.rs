// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Spawn configuration and resource limits for colony workers.
//!
//! This module defines the configuration for spawning new DashFlow instances,
//! including templates, resource limits, and deployment options.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use super::topology::DeploymentOption;
use super::types::SpawnRequirements;

/// Configuration for spawning a new worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnConfig {
    /// Template to use for spawning
    pub template: SpawnTemplate,

    /// Task for the worker to execute
    pub task: Task,

    /// Resource limits for the spawned worker
    pub resources: ResourceLimits,

    /// Deployment preference
    pub deployment: DeploymentOption,

    /// Whether to terminate automatically when task completes
    pub auto_terminate: bool,

    /// Termination policy
    pub termination_policy: TerminationPolicy,

    /// Environment variables to pass to the worker
    pub environment: HashMap<String, String>,

    /// Working directory for the worker
    pub working_directory: Option<PathBuf>,

    /// Priority of this worker (lower = higher priority)
    pub priority: u8,

    /// Optional name for the worker
    pub name: Option<String>,

    /// Tags for categorization
    pub tags: Vec<String>,
}

impl Default for SpawnConfig {
    fn default() -> Self {
        Self {
            template: SpawnTemplate::Self_,
            task: Task::Idle,
            resources: ResourceLimits::default(),
            deployment: DeploymentOption::Any,
            auto_terminate: true,
            termination_policy: TerminationPolicy::WhenTaskComplete,
            environment: HashMap::new(),
            working_directory: None,
            priority: 100, // Default priority
            name: None,
            tags: Vec::new(),
        }
    }
}

impl SpawnConfig {
    /// Create a new spawn config with a task.
    #[must_use]
    pub fn with_task(task: Task) -> Self {
        Self {
            task,
            ..Default::default()
        }
    }

    /// Builder: set template
    pub fn template(mut self, template: SpawnTemplate) -> Self {
        self.template = template;
        self
    }

    /// Builder: set resource limits
    pub fn resources(mut self, resources: ResourceLimits) -> Self {
        self.resources = resources;
        self
    }

    /// Builder: set deployment option
    pub fn deployment(mut self, deployment: DeploymentOption) -> Self {
        self.deployment = deployment;
        self
    }

    /// Builder: set auto-terminate
    pub fn auto_terminate(mut self, auto_terminate: bool) -> Self {
        self.auto_terminate = auto_terminate;
        self
    }

    /// Builder: set termination policy
    pub fn termination_policy(mut self, policy: TerminationPolicy) -> Self {
        self.termination_policy = policy;
        self
    }

    /// Builder: add environment variable
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.environment.insert(key.into(), value.into());
        self
    }

    /// Builder: set working directory
    pub fn working_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.working_directory = Some(path.into());
        self
    }

    /// Builder: set priority
    pub fn priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    /// Builder: set name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Builder: add tag
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Check if this config's requirements can be satisfied by the given resources.
    pub fn can_spawn_with(&self, available: &SpawnRequirements) -> bool {
        available.min_cpu_cores <= self.resources.max_cpu_cores.unwrap_or(u32::MAX)
            && available.min_memory_mb <= self.resources.max_memory_mb.unwrap_or(u64::MAX)
            && available.min_disk_mb <= self.resources.max_disk_mb.unwrap_or(u64::MAX)
    }

    /// Get the spawn requirements for this config.
    pub fn requirements(&self) -> SpawnRequirements {
        SpawnRequirements {
            min_cpu_cores: self.resources.min_cpu_cores.unwrap_or(1),
            min_memory_mb: self.resources.min_memory_mb.unwrap_or(512),
            min_disk_mb: self.resources.min_disk_mb.unwrap_or(256),
            min_llm_rate_limit: 0, // Determined by task
            min_gpu_vram_gb: self
                .resources
                .require_gpu
                .then(|| self.resources.min_gpu_vram_gb.unwrap_or(0)),
        }
    }
}

/// Template for spawning a worker.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum SpawnTemplate {
    /// Clone of the current app
    #[default]
    Self_,

    /// Named template from the registry
    Named(String),

    /// Custom application configuration
    Custom(Box<AppConfig>),
}

impl SpawnTemplate {
    /// Create a named template.
    pub fn named(name: impl Into<String>) -> Self {
        Self::Named(name.into())
    }

    /// Create a custom template.
    pub fn custom(config: AppConfig) -> Self {
        Self::Custom(Box::new(config))
    }

    /// Get the template name (for display/logging).
    pub fn name(&self) -> &str {
        match self {
            Self::Self_ => "self",
            Self::Named(name) => name,
            Self::Custom(config) => &config.name,
        }
    }
}

/// Custom application configuration for spawning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Application name
    pub name: String,

    /// Path to the executable
    pub executable: PathBuf,

    /// Command line arguments
    pub args: Vec<String>,

    /// Environment variables
    pub environment: HashMap<String, String>,

    /// Working directory
    pub working_directory: Option<PathBuf>,

    /// Description of the app
    pub description: Option<String>,

    /// Capabilities provided by this app
    pub capabilities: Vec<String>,

    /// Default resource limits
    pub default_resources: ResourceLimits,

    /// Docker configuration (if applicable)
    pub docker: Option<DockerConfig>,
}

impl AppConfig {
    /// Create a new app config.
    pub fn new(name: impl Into<String>, executable: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            executable: executable.into(),
            args: Vec::new(),
            environment: HashMap::new(),
            working_directory: None,
            description: None,
            capabilities: Vec::new(),
            default_resources: ResourceLimits::default(),
            docker: None,
        }
    }

    /// Builder: add argument
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Builder: add environment variable
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.environment.insert(key.into(), value.into());
        self
    }

    /// Builder: set working directory
    pub fn working_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.working_directory = Some(path.into());
        self
    }

    /// Builder: set description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Builder: add capability
    pub fn capability(mut self, cap: impl Into<String>) -> Self {
        self.capabilities.push(cap.into());
        self
    }

    /// Builder: set docker config
    pub fn docker_config(mut self, docker: DockerConfig) -> Self {
        self.docker = Some(docker);
        self
    }
}

/// Docker-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerConfig {
    /// Docker image to use
    pub image: String,

    /// Image tag
    pub tag: String,

    /// Network mode (host, bridge, none)
    pub network: DockerNetwork,

    /// Volume mounts
    pub volumes: Vec<VolumeMount>,

    /// Port mappings
    pub ports: Vec<PortMapping>,

    /// Privileged mode
    pub privileged: bool,

    /// Additional docker run arguments
    pub extra_args: Vec<String>,
}

impl DockerConfig {
    /// Create a new docker config.
    pub fn new(image: impl Into<String>) -> Self {
        Self {
            image: image.into(),
            tag: "latest".to_string(),
            network: DockerNetwork::Host,
            volumes: Vec::new(),
            ports: Vec::new(),
            privileged: false,
            extra_args: Vec::new(),
        }
    }

    /// Builder: set tag
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = tag.into();
        self
    }

    /// Builder: set network
    pub fn network(mut self, network: DockerNetwork) -> Self {
        self.network = network;
        self
    }

    /// Builder: add volume mount
    pub fn volume(mut self, mount: VolumeMount) -> Self {
        self.volumes.push(mount);
        self
    }

    /// Builder: add port mapping
    pub fn port(mut self, mapping: PortMapping) -> Self {
        self.ports.push(mapping);
        self
    }

    /// Get the full image reference (image:tag)
    pub fn image_ref(&self) -> String {
        format!("{}:{}", self.image, self.tag)
    }
}

/// Docker network mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DockerNetwork {
    /// Host networking
    #[default]
    Host,

    /// Bridge networking
    Bridge,

    /// No networking
    None,

    /// Custom network name
    Custom,
}

/// Volume mount for Docker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeMount {
    /// Host path
    pub host_path: PathBuf,

    /// Container path
    pub container_path: PathBuf,

    /// Read-only mount
    pub read_only: bool,
}

impl VolumeMount {
    /// Create a new volume mount.
    pub fn new(host: impl Into<PathBuf>, container: impl Into<PathBuf>) -> Self {
        Self {
            host_path: host.into(),
            container_path: container.into(),
            read_only: false,
        }
    }

    /// Create a read-only volume mount.
    pub fn read_only(host: impl Into<PathBuf>, container: impl Into<PathBuf>) -> Self {
        Self {
            host_path: host.into(),
            container_path: container.into(),
            read_only: true,
        }
    }
}

/// Port mapping for Docker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortMapping {
    /// Host port
    pub host_port: u16,

    /// Container port
    pub container_port: u16,

    /// Protocol (tcp, udp)
    pub protocol: PortProtocol,
}

impl PortMapping {
    /// Create a new TCP port mapping.
    pub fn tcp(host: u16, container: u16) -> Self {
        Self {
            host_port: host,
            container_port: container,
            protocol: PortProtocol::Tcp,
        }
    }

    /// Create a new UDP port mapping.
    pub fn udp(host: u16, container: u16) -> Self {
        Self {
            host_port: host,
            container_port: container,
            protocol: PortProtocol::Udp,
        }
    }
}

/// Port protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PortProtocol {
    /// TCP protocol (default).
    #[default]
    Tcp,
    /// UDP protocol.
    Udp,
}

/// Resource limits for a spawned worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum CPU cores
    pub max_cpu_cores: Option<u32>,

    /// Minimum CPU cores (guaranteed)
    pub min_cpu_cores: Option<u32>,

    /// Maximum memory in MB
    pub max_memory_mb: Option<u64>,

    /// Minimum memory in MB (guaranteed)
    pub min_memory_mb: Option<u64>,

    /// Maximum disk usage in MB
    pub max_disk_mb: Option<u64>,

    /// Minimum disk space in MB (required)
    pub min_disk_mb: Option<u64>,

    /// Maximum execution duration
    pub max_duration: Option<Duration>,

    /// Maximum number of spawned sub-workers
    pub max_sub_workers: Option<u32>,

    /// GPU required
    pub require_gpu: bool,

    /// Minimum GPU VRAM in GB (if GPU required)
    pub min_gpu_vram_gb: Option<u32>,

    /// Network access allowed
    pub network_access: bool,

    /// Filesystem access level
    pub filesystem_access: FilesystemAccess,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_cpu_cores: None,
            min_cpu_cores: Some(1),
            max_memory_mb: None,
            min_memory_mb: Some(512),
            max_disk_mb: None,
            min_disk_mb: Some(256),
            max_duration: Some(Duration::from_secs(3600)), // 1 hour default
            max_sub_workers: Some(5),
            require_gpu: false,
            min_gpu_vram_gb: None,
            network_access: true,
            filesystem_access: FilesystemAccess::ReadWrite,
        }
    }
}

impl ResourceLimits {
    /// Create minimal resource limits (for lightweight workers).
    pub fn minimal() -> Self {
        Self {
            max_cpu_cores: Some(1),
            min_cpu_cores: Some(1),
            max_memory_mb: Some(512),
            min_memory_mb: Some(256),
            max_disk_mb: Some(256),
            min_disk_mb: Some(128),
            max_duration: Some(Duration::from_secs(300)), // 5 minutes
            max_sub_workers: Some(0),
            require_gpu: false,
            min_gpu_vram_gb: None,
            network_access: true,
            filesystem_access: FilesystemAccess::ReadOnly,
        }
    }

    /// Create standard resource limits.
    pub fn standard() -> Self {
        Self {
            max_cpu_cores: Some(2),
            min_cpu_cores: Some(1),
            max_memory_mb: Some(4096),
            min_memory_mb: Some(1024),
            max_disk_mb: Some(10240),
            min_disk_mb: Some(1024),
            max_duration: Some(Duration::from_secs(3600)), // 1 hour
            max_sub_workers: Some(3),
            require_gpu: false,
            min_gpu_vram_gb: None,
            network_access: true,
            filesystem_access: FilesystemAccess::ReadWrite,
        }
    }

    /// Create heavy resource limits (for builds, optimization).
    pub fn heavy() -> Self {
        Self {
            max_cpu_cores: Some(8),
            min_cpu_cores: Some(4),
            max_memory_mb: Some(16384),
            min_memory_mb: Some(8192),
            max_disk_mb: Some(102400),
            min_disk_mb: Some(10240),
            max_duration: Some(Duration::from_secs(14400)), // 4 hours
            max_sub_workers: Some(5),
            require_gpu: false,
            min_gpu_vram_gb: None,
            network_access: true,
            filesystem_access: FilesystemAccess::ReadWrite,
        }
    }

    /// Create unlimited resource limits (for trusted workers).
    pub fn unlimited() -> Self {
        Self {
            max_cpu_cores: None,
            min_cpu_cores: None,
            max_memory_mb: None,
            min_memory_mb: None,
            max_disk_mb: None,
            min_disk_mb: None,
            max_duration: None,
            max_sub_workers: None,
            require_gpu: false,
            min_gpu_vram_gb: None,
            network_access: true,
            filesystem_access: FilesystemAccess::Full,
        }
    }

    /// Builder: set max CPU
    pub fn max_cpu(mut self, cores: u32) -> Self {
        self.max_cpu_cores = Some(cores);
        self
    }

    /// Builder: set max memory
    pub fn max_memory(mut self, mb: u64) -> Self {
        self.max_memory_mb = Some(mb);
        self
    }

    /// Builder: set max duration
    pub fn max_duration(mut self, duration: Duration) -> Self {
        self.max_duration = Some(duration);
        self
    }

    /// Builder: remove duration limit (unlimited)
    pub fn no_duration_limit(mut self) -> Self {
        self.max_duration = None;
        self
    }

    /// Builder: require GPU
    #[must_use]
    pub fn with_gpu(mut self, min_vram_gb: u32) -> Self {
        self.require_gpu = true;
        self.min_gpu_vram_gb = Some(min_vram_gb);
        self
    }

    /// Builder: set filesystem access
    pub fn filesystem(mut self, access: FilesystemAccess) -> Self {
        self.filesystem_access = access;
        self
    }
}

/// Filesystem access level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FilesystemAccess {
    /// No filesystem access
    None,

    /// Read-only access
    ReadOnly,

    /// Read-write access (default)
    #[default]
    ReadWrite,

    /// Full access including system directories
    Full,
}

impl FilesystemAccess {
    /// Check if reading is allowed.
    pub fn can_read(&self) -> bool {
        !matches!(self, Self::None)
    }

    /// Check if writing is allowed.
    pub fn can_write(&self) -> bool {
        matches!(self, Self::ReadWrite | Self::Full)
    }
}

/// Task to be executed by a worker.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum Task {
    /// Worker is idle (waiting for commands)
    #[default]
    Idle,

    /// Run tests for a crate
    RunTests {
        /// Name of the crate to test.
        crate_name: String,
        /// Optional test name filter pattern.
        test_filter: Option<String>,
    },

    /// Build a target
    Build {
        /// Target name to build.
        target: String,
        /// Build in release mode.
        release: bool,
    },

    /// Run optimization
    Optimize {
        /// Target to optimize.
        target: String,
        /// Number of optimization iterations.
        iterations: u32,
    },

    /// Analyze code
    Analyze {
        /// Path to analyze.
        path: PathBuf,
        /// Type of analysis to perform.
        analysis_type: AnalysisType,
    },

    /// Execute a shell command
    Command {
        /// The command to execute.
        command: String,
        /// Arguments to pass to the command.
        args: Vec<String>,
    },

    /// Custom task with arbitrary data
    Custom(serde_json::Value),
}

impl Task {
    /// Create a test task.
    pub fn test(crate_name: impl Into<String>) -> Self {
        Self::RunTests {
            crate_name: crate_name.into(),
            test_filter: None,
        }
    }

    /// Create a build task.
    pub fn build(target: impl Into<String>) -> Self {
        Self::Build {
            target: target.into(),
            release: false,
        }
    }

    /// Create an optimization task.
    pub fn optimize(target: impl Into<String>, iterations: u32) -> Self {
        Self::Optimize {
            target: target.into(),
            iterations,
        }
    }

    /// Create a command task.
    pub fn command(command: impl Into<String>) -> Self {
        Self::Command {
            command: command.into(),
            args: Vec::new(),
        }
    }

    /// Get the task type name.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::RunTests { .. } => "run_tests",
            Self::Build { .. } => "build",
            Self::Optimize { .. } => "optimize",
            Self::Analyze { .. } => "analyze",
            Self::Command { .. } => "command",
            Self::Custom(_) => "custom",
        }
    }

    /// Get recommended resource limits for this task.
    pub fn recommended_resources(&self) -> ResourceLimits {
        match self {
            Self::Idle => ResourceLimits::minimal(),
            Self::RunTests { .. } => ResourceLimits::standard(),
            Self::Build { release, .. } => {
                if *release {
                    ResourceLimits::heavy()
                } else {
                    ResourceLimits::standard()
                }
            }
            Self::Optimize { .. } => ResourceLimits::heavy(),
            Self::Analyze { .. } => ResourceLimits::standard(),
            Self::Command { .. } => ResourceLimits::standard(),
            Self::Custom(_) => ResourceLimits::standard(),
        }
    }
}

/// Type of code analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnalysisType {
    /// Static analysis
    Static,

    /// Security analysis
    Security,

    /// Performance analysis
    Performance,

    /// Dependency analysis
    Dependencies,

    /// Code quality
    Quality,
}

/// When to terminate a worker.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum TerminationPolicy {
    /// Terminate when task is complete
    #[default]
    WhenTaskComplete,

    /// Terminate after being idle for a duration
    WhenIdle(Duration),

    /// Only terminate via explicit command
    Manual,

    /// Terminate if parent process exits
    WhenParentExits,

    /// Terminate after a fixed duration
    AfterDuration(Duration),
}

/// Spawn approval mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SpawnApproval {
    /// Automatically spawn if within limits
    #[default]
    Automatic,

    /// Require user confirmation before spawning
    RequireConfirm,

    /// Spawning is disabled
    Disabled,
}

/// Global colony configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColonyConfig {
    /// Spawn approval mode
    pub approval: SpawnApproval,

    /// Maximum number of workers
    pub max_workers: u32,

    /// Global resource limits
    pub resource_limits: GlobalResourceLimits,

    /// Allowed spawn templates
    pub allowed_templates: Option<Vec<String>>,

    /// Banned spawn templates
    pub banned_templates: Vec<String>,

    /// Enable audit logging
    pub audit_log: bool,
}

impl Default for ColonyConfig {
    fn default() -> Self {
        Self {
            approval: SpawnApproval::Automatic,
            max_workers: 10,
            resource_limits: GlobalResourceLimits::default(),
            allowed_templates: None, // All allowed
            banned_templates: Vec::new(),
            audit_log: true,
        }
    }
}

impl ColonyConfig {
    /// Check if a template is allowed.
    pub fn is_template_allowed(&self, template: &str) -> bool {
        // Check banned list first
        if self.banned_templates.contains(&template.to_string()) {
            return false;
        }

        // If allowed list is set, check it
        if let Some(ref allowed) = self.allowed_templates {
            return allowed.contains(&template.to_string());
        }

        // Otherwise all templates are allowed
        true
    }
}

/// Global resource limits for the colony.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalResourceLimits {
    /// Maximum percentage of CPU to use for workers
    pub max_cpu_percent: f32,

    /// Maximum percentage of memory to use for workers
    pub max_memory_percent: f32,

    /// Minimum reserved memory in MB (always keep free)
    pub min_reserved_memory_mb: u64,

    /// Minimum reserved disk in MB (always keep free)
    pub min_reserved_disk_mb: u64,
}

impl Default for GlobalResourceLimits {
    fn default() -> Self {
        Self {
            max_cpu_percent: 80.0,
            max_memory_percent: 80.0,
            min_reserved_memory_mb: 4096, // Always keep 4GB free
            min_reserved_disk_mb: 10240,  // Always keep 10GB free
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spawn_config_builder() {
        let config = SpawnConfig::with_task(Task::test("dashflow-openai"))
            .template(SpawnTemplate::named("test-runner"))
            .resources(ResourceLimits::standard())
            .priority(50)
            .name("test-worker")
            .tag("critical")
            .env("RUST_LOG", "debug");

        assert_eq!(config.name, Some("test-worker".to_string()));
        assert_eq!(config.priority, 50);
        assert!(config.tags.contains(&"critical".to_string()));
        assert_eq!(
            config.environment.get("RUST_LOG"),
            Some(&"debug".to_string())
        );
    }

    #[test]
    fn test_spawn_template() {
        assert_eq!(SpawnTemplate::Self_.name(), "self");
        assert_eq!(SpawnTemplate::named("test-runner").name(), "test-runner");

        let custom = SpawnTemplate::custom(AppConfig::new("my-app", "/usr/bin/app"));
        assert_eq!(custom.name(), "my-app");
    }

    #[test]
    fn test_resource_limits_presets() {
        let minimal = ResourceLimits::minimal();
        assert_eq!(minimal.max_cpu_cores, Some(1));
        assert_eq!(minimal.max_memory_mb, Some(512));
        assert_eq!(minimal.filesystem_access, FilesystemAccess::ReadOnly);

        let standard = ResourceLimits::standard();
        assert_eq!(standard.max_cpu_cores, Some(2));
        assert_eq!(standard.max_memory_mb, Some(4096));

        let heavy = ResourceLimits::heavy();
        assert_eq!(heavy.max_cpu_cores, Some(8));
        assert_eq!(heavy.max_memory_mb, Some(16384));

        let unlimited = ResourceLimits::unlimited();
        assert_eq!(unlimited.max_cpu_cores, None);
        assert_eq!(unlimited.max_memory_mb, None);
        assert_eq!(unlimited.filesystem_access, FilesystemAccess::Full);
    }

    #[test]
    fn test_resource_limits_builder() {
        let limits = ResourceLimits::standard()
            .max_cpu(4)
            .max_memory(8192)
            .max_duration(Duration::from_secs(7200))
            .with_gpu(8)
            .filesystem(FilesystemAccess::Full);

        assert_eq!(limits.max_cpu_cores, Some(4));
        assert_eq!(limits.max_memory_mb, Some(8192));
        assert_eq!(limits.max_duration, Some(Duration::from_secs(7200)));
        assert!(limits.require_gpu);
        assert_eq!(limits.min_gpu_vram_gb, Some(8));
        assert_eq!(limits.filesystem_access, FilesystemAccess::Full);
    }

    #[test]
    fn test_filesystem_access() {
        assert!(!FilesystemAccess::None.can_read());
        assert!(!FilesystemAccess::None.can_write());

        assert!(FilesystemAccess::ReadOnly.can_read());
        assert!(!FilesystemAccess::ReadOnly.can_write());

        assert!(FilesystemAccess::ReadWrite.can_read());
        assert!(FilesystemAccess::ReadWrite.can_write());

        assert!(FilesystemAccess::Full.can_read());
        assert!(FilesystemAccess::Full.can_write());
    }

    #[test]
    fn test_task_types() {
        assert_eq!(Task::Idle.type_name(), "idle");
        assert_eq!(Task::test("foo").type_name(), "run_tests");
        assert_eq!(Task::build("release").type_name(), "build");
        assert_eq!(Task::optimize("grpo", 100).type_name(), "optimize");
        assert_eq!(Task::command("cargo").type_name(), "command");
    }

    #[test]
    fn test_task_recommended_resources() {
        let idle_resources = Task::Idle.recommended_resources();
        assert_eq!(idle_resources.max_cpu_cores, Some(1));

        let build_resources = Task::Build {
            target: "release".to_string(),
            release: true,
        }
        .recommended_resources();
        assert_eq!(build_resources.max_cpu_cores, Some(8)); // Heavy for release
    }

    #[test]
    fn test_termination_policy() {
        assert!(matches!(
            TerminationPolicy::default(),
            TerminationPolicy::WhenTaskComplete
        ));
    }

    #[test]
    fn test_colony_config_template_allowed() {
        let mut config = ColonyConfig::default();

        // All templates allowed by default
        assert!(config.is_template_allowed("test-runner"));
        assert!(config.is_template_allowed("anything"));

        // Ban a template
        config.banned_templates.push("dangerous".to_string());
        assert!(!config.is_template_allowed("dangerous"));
        assert!(config.is_template_allowed("test-runner"));

        // Set allowed list
        config.allowed_templates = Some(vec!["test-runner".to_string()]);
        assert!(config.is_template_allowed("test-runner"));
        assert!(!config.is_template_allowed("anything"));
    }

    #[test]
    fn test_docker_config() {
        let config = DockerConfig::new("dashflow")
            .tag("v1.0.0")
            .network(DockerNetwork::Bridge)
            .volume(VolumeMount::new("/host/data", "/data"))
            .port(PortMapping::tcp(8080, 80));

        assert_eq!(config.image_ref(), "dashflow:v1.0.0");
        assert_eq!(config.network, DockerNetwork::Bridge);
        assert_eq!(config.volumes.len(), 1);
        assert_eq!(config.ports.len(), 1);
    }

    #[test]
    fn test_app_config_builder() {
        let config = AppConfig::new("my-app", "/usr/bin/app")
            .arg("--verbose")
            .env("APP_MODE", "production")
            .working_dir("/var/app")
            .description("My application")
            .capability("testing");

        assert_eq!(config.name, "my-app");
        assert_eq!(config.args, vec!["--verbose"]);
        assert_eq!(
            config.environment.get("APP_MODE"),
            Some(&"production".to_string())
        );
        assert_eq!(config.working_directory, Some(PathBuf::from("/var/app")));
        assert!(config.capabilities.contains(&"testing".to_string()));
    }

    #[test]
    fn test_spawn_config_requirements() {
        let config = SpawnConfig::default().resources(ResourceLimits::standard());

        let requirements = config.requirements();
        assert_eq!(requirements.min_cpu_cores, 1);
        assert_eq!(requirements.min_memory_mb, 1024);
    }
}
