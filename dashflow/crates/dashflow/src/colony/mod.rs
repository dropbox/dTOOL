// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! @dashflow-module
//! @name colony
//! @category runtime
//! @status stable
//!
//! # Colony Module - Organic Spawning and Resource Management
//!
//! DashFlow apps can introspect system resources and architecture, then spawn new
//! DashFlow instances when resources are available. Like an ant or bee colony that
//! grows by adding specialized workers, DashFlow apps expand their capabilities by
//! spawning task-specific agents.
//!
//! ## Key Capabilities
//!
//! - **System Introspection**: Query CPU, memory, disk, and network resources
//! - **LLM Service Tracking**: Monitor rate limits, latency, and health across providers
//! - **Topology Awareness**: Understand system architecture and deployment options
//! - **Spawn Configuration**: Configure workers with templates, resource limits, and tasks
//! - **Template Registry**: Predefined templates for common worker types
//! - **Organic Spawning**: Create worker processes when resources permit
//! - **Worker Lifecycle**: State machine for worker lifecycle management
//! - **Colony Coordination**: Workers communicate via network module
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::colony::{SystemMonitor, SpawnRequirements, SpawnConfig, Task};
//! use dashflow::colony::templates::quick;
//!
//! // Check resources
//! let monitor = SystemMonitor::new()?;
//! let resources = monitor.available_resources().await?;
//!
//! if resources.can_spawn(SpawnRequirements::standard()) {
//!     // Create a test runner worker
//!     let config = quick::test_runner("dashflow-openai");
//!     // ... spawn the worker
//! }
//! ```
//!
//! ## Templates
//!
//! ```rust,ignore
//! use dashflow::colony::templates::{quick, names, get_template};
//!
//! // Use quick helpers for common tasks
//! let test_config = quick::test_runner("my-crate");
//! let build_config = quick::builder("release", true);
//! let opt_config = quick::optimizer("grpo", 100);
//!
//! // Or get templates directly
//! if let Some(template) = get_template(names::BENCHMARKER) {
//!     let config = template.to_spawn_config(Task::command("cargo bench"));
//! }
//! ```

mod config;
mod network_integration;
mod spawner;
mod system;
pub mod templates;
mod topology;
mod types;

pub use config::{
    AnalysisType, AppConfig, ColonyConfig, DockerConfig, DockerNetwork, FilesystemAccess,
    GlobalResourceLimits, PortMapping, PortProtocol, ResourceLimits, SpawnApproval, SpawnConfig,
    SpawnTemplate, Task, TerminationPolicy, VolumeMount,
};
#[cfg(feature = "network")]
pub use network_integration::{
    announce_worker_joined, report_progress, report_result, NetworkedSpawner,
    NetworkedWorkerManager,
};
pub use network_integration::{
    is_worker_mode, network_enabled, parent_peer_id, worker_id, worker_task, NetworkedWorkerInfo,
    WorkerMessage, WORKER_CHANNEL, WORKER_JOIN_CHANNEL, WORKER_RESULT_CHANNEL,
};
pub use spawner::{
    SpawnError, Spawner, TaskResult, Worker, WorkerInfo, WorkerManager, WorkerState,
};
pub use system::{SystemMonitor, SystemMonitorConfig, SystemMonitorError};
pub use templates::{TemplateDefinition, TemplateRegistry};
pub use topology::{
    ContainerRuntime, DeploymentOption, NetworkInterface, NumaNode, SpawnOption, SystemTopology,
};
pub use types::{
    CostPer1k, DiskInfo, GpuInfo, LlmServiceStats, MemoryInfo, ResourceSnapshot, SpawnRequirements,
    TotalLlmCapacity,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all types are exported correctly
        let _: Option<SystemMonitor> = None;
        let _: Option<SystemTopology> = None;
        let _: Option<ResourceSnapshot> = None;
        let _: Option<SpawnRequirements> = None;

        // Feature exports
        let _: Option<SpawnConfig> = None;
        let _: Option<SpawnTemplate> = None;
        let _: Option<ResourceLimits> = None;
        let _: Option<Task> = None;
        let _: Option<TerminationPolicy> = None;
        let _: Option<SpawnApproval> = None;
        let _: Option<ColonyConfig> = None;
        let _: Option<AppConfig> = None;
        let _: Option<DockerConfig> = None;
        let _: Option<TemplateRegistry> = None;
        let _: Option<TemplateDefinition> = None;

        // Feature exports
        let _: Option<Worker> = None;
        let _: Option<WorkerState> = None;
        let _: Option<WorkerInfo> = None;
        let _: Option<WorkerManager> = None;
        let _: Option<Spawner> = None;
        let _: Option<TaskResult> = None;
        let _: Option<SpawnError> = None;
    }

    #[test]
    fn test_spawn_workflow() {
        // Simulate the spawn workflow

        // 1. Check if we have resources
        let requirements = SpawnRequirements::standard();
        assert_eq!(requirements.min_cpu_cores, 2);
        assert_eq!(requirements.min_memory_mb, 2048);

        // 2. Create a spawn config
        let config = SpawnConfig::with_task(Task::test("dashflow-openai"))
            .resources(ResourceLimits::standard())
            .name("test-worker");

        assert!(config.auto_terminate);
        assert_eq!(config.name, Some("test-worker".to_string()));

        // 3. Check requirements match
        let spawn_requirements = config.requirements();
        assert_eq!(spawn_requirements.min_cpu_cores, 1);
    }

    #[test]
    fn test_template_integration() {
        // Test that templates module is properly integrated
        use templates::{get_template, names, quick, template_exists};

        // Quick helpers
        let test_config = quick::test_runner("my-crate");
        assert!(matches!(test_config.task, Task::RunTests { .. }));

        // Template lookup
        assert!(template_exists(names::TEST_RUNNER));
        assert!(template_exists(names::BUILDER));
        assert!(template_exists(names::OPTIMIZER));

        // Get template
        let template = get_template(names::ANALYZER).unwrap();
        assert_eq!(template.name, names::ANALYZER);
    }

    #[test]
    fn test_worker_lifecycle_integration() {
        // Test worker state machine integration
        let config = SpawnConfig::with_task(Task::test("dashflow-openai"))
            .name("test-worker")
            .resources(ResourceLimits::minimal());

        let worker = Worker::new(config);

        assert_eq!(worker.name, Some("test-worker".to_string()));
        assert!(!worker.is_active());
        assert!(!worker.is_terminal());
        assert!(matches!(worker.state(), WorkerState::Planned { .. }));
    }

    #[test]
    fn test_task_result_integration() {
        use std::time::Duration;

        let result = TaskResult::success(
            0,
            "Tests passed".to_string(),
            "".to_string(),
            Duration::from_secs(30),
        );

        assert!(result.success);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout, "Tests passed");
    }
}
