// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for colony templates
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]

//! Predefined spawn templates for common worker types.
//!
//! Templates provide ready-to-use configurations for spawning workers
//! with appropriate resource limits and capabilities.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use crate::registry_trait::Registry;

use super::config::{
    AnalysisType, FilesystemAccess, ResourceLimits, SpawnConfig, SpawnTemplate, Task,
    TerminationPolicy,
};
use super::topology::DeploymentOption;

/// Registry of available spawn templates.
pub struct TemplateRegistry {
    templates: HashMap<String, TemplateDefinition>,
}

/// Definition of a spawn template.
#[derive(Debug, Clone)]
pub struct TemplateDefinition {
    /// Template name
    pub name: String,

    /// Human-readable description
    pub description: String,

    /// Default resource limits
    pub default_resources: ResourceLimits,

    /// Capabilities this template provides
    pub capabilities: Vec<String>,

    /// Default termination policy
    pub default_termination: TerminationPolicy,

    /// Default deployment preference
    pub default_deployment: DeploymentOption,

    /// Whether this template supports sub-spawning
    pub can_spawn_workers: bool,

    /// Tags for categorization
    pub tags: Vec<String>,
}

impl TemplateDefinition {
    /// Create a new template definition.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            default_resources: ResourceLimits::standard(),
            capabilities: Vec::new(),
            default_termination: TerminationPolicy::WhenTaskComplete,
            default_deployment: DeploymentOption::Any,
            can_spawn_workers: false,
            tags: Vec::new(),
        }
    }

    /// Builder: set default resources
    pub fn resources(mut self, resources: ResourceLimits) -> Self {
        self.default_resources = resources;
        self
    }

    /// Builder: add capability
    pub fn capability(mut self, cap: impl Into<String>) -> Self {
        self.capabilities.push(cap.into());
        self
    }

    /// Builder: set termination policy
    pub fn termination(mut self, policy: TerminationPolicy) -> Self {
        self.default_termination = policy;
        self
    }

    /// Builder: set deployment preference
    pub fn deployment(mut self, deployment: DeploymentOption) -> Self {
        self.default_deployment = deployment;
        self
    }

    /// Builder: allow spawning workers
    pub fn allow_spawn(mut self) -> Self {
        self.can_spawn_workers = true;
        self
    }

    /// Builder: add tag
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Convert to a SpawnConfig for a given task.
    pub fn to_spawn_config(&self, task: Task) -> SpawnConfig {
        SpawnConfig {
            template: SpawnTemplate::Named(self.name.clone()),
            task,
            resources: self.default_resources.clone(),
            deployment: self.default_deployment,
            auto_terminate: true,
            termination_policy: self.default_termination.clone(),
            environment: HashMap::new(),
            working_directory: None,
            priority: 100,
            name: None,
            tags: self.tags.clone(),
        }
    }
}

/// Predefined template names.
pub mod names {
    /// Test runner template
    pub const TEST_RUNNER: &str = "test-runner";

    /// Build worker template
    pub const BUILDER: &str = "builder";

    /// Code optimizer template
    pub const OPTIMIZER: &str = "optimizer";

    /// Code analyzer template
    pub const ANALYZER: &str = "analyzer";

    /// General worker template
    pub const WORKER: &str = "worker";

    /// Scout/explorer template
    pub const SCOUT: &str = "scout";

    /// Benchmark runner template
    pub const BENCHMARKER: &str = "benchmarker";

    /// Documentation generator template
    pub const DOC_GENERATOR: &str = "doc-generator";

    /// Linter/formatter template
    pub const LINTER: &str = "linter";

    /// Watcher template (monitors for changes)
    pub const WATCHER: &str = "watcher";
}

/// Get the default template registry with all predefined templates.
static DEFAULT_REGISTRY: OnceLock<TemplateRegistry> = OnceLock::new();

/// Get a reference to the default template registry.
fn default_registry() -> &'static TemplateRegistry {
    DEFAULT_REGISTRY.get_or_init(|| {
        let mut registry = TemplateRegistry::new();
        registry.register_defaults();
        registry
    })
}

impl TemplateRegistry {
    /// Create a new empty template registry.
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
        }
    }

    /// Create a registry with default templates.
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register_defaults();
        registry
    }

    /// Register a template.
    pub fn register(&mut self, template: TemplateDefinition) {
        self.templates.insert(template.name.clone(), template);
    }

    /// Get a template by name.
    pub fn get(&self, name: &str) -> Option<&TemplateDefinition> {
        self.templates.get(name)
    }

    /// Check if a template exists.
    pub fn contains(&self, name: &str) -> bool {
        self.templates.contains_key(name)
    }

    /// Get all template names.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.templates.keys().map(|s| s.as_str())
    }

    /// Get all templates.
    pub fn templates(&self) -> impl Iterator<Item = &TemplateDefinition> {
        self.templates.values()
    }

    /// Get templates by tag.
    pub fn by_tag(&self, tag: &str) -> Vec<&TemplateDefinition> {
        self.templates
            .values()
            .filter(|t| t.tags.iter().any(|t_tag| t_tag == tag))
            .collect()
    }

    /// Get templates with a specific capability.
    pub fn with_capability(&self, capability: &str) -> Vec<&TemplateDefinition> {
        self.templates
            .values()
            .filter(|t| t.capabilities.iter().any(|c| c == capability))
            .collect()
    }

    /// Register all default templates.
    fn register_defaults(&mut self) {
        // Test runner - runs tests for crates
        self.register(
            TemplateDefinition::new(names::TEST_RUNNER, "Runs tests for a crate or project")
                .resources(
                    ResourceLimits::standard()
                        .max_cpu(4)
                        .max_memory(8192)
                        .max_duration(Duration::from_secs(1800)), // 30 min
                )
                .capability("testing")
                .capability("cargo")
                .termination(TerminationPolicy::WhenTaskComplete)
                .tag("testing")
                .tag("ci"),
        );

        // Builder - builds targets
        self.register(
            TemplateDefinition::new(names::BUILDER, "Builds release or debug targets")
                .resources(
                    ResourceLimits::heavy().max_duration(Duration::from_secs(3600)), // 1 hour
                )
                .capability("building")
                .capability("cargo")
                .capability("compilation")
                .termination(TerminationPolicy::WhenTaskComplete)
                .tag("building")
                .tag("ci"),
        );

        // Optimizer - runs optimization tasks
        self.register(
            TemplateDefinition::new(names::OPTIMIZER, "Runs optimization and tuning tasks")
                .resources(
                    ResourceLimits::heavy().max_duration(Duration::from_secs(14400)), // 4 hours
                )
                .capability("optimization")
                .capability("ml")
                .capability("tuning")
                .termination(TerminationPolicy::WhenTaskComplete)
                .allow_spawn() // Can spawn sub-workers
                .tag("optimization")
                .tag("compute-heavy"),
        );

        // Analyzer - analyzes code
        self.register(
            TemplateDefinition::new(names::ANALYZER, "Analyzes code for various metrics")
                .resources(
                    ResourceLimits::standard()
                        .max_cpu(2)
                        .max_memory(4096)
                        .max_duration(Duration::from_secs(1800))
                        .filesystem(FilesystemAccess::ReadOnly),
                )
                .capability("analysis")
                .capability("static-analysis")
                .capability("security")
                .termination(TerminationPolicy::WhenTaskComplete)
                .tag("analysis")
                .tag("read-only"),
        );

        // Worker - general purpose worker
        self.register(
            TemplateDefinition::new(names::WORKER, "General purpose worker for various tasks")
                .resources(ResourceLimits::standard())
                .capability("general")
                .termination(TerminationPolicy::WhenIdle(Duration::from_secs(300)))
                .allow_spawn()
                .tag("general"),
        );

        // Scout - lightweight explorer
        self.register(
            TemplateDefinition::new(
                names::SCOUT,
                "Lightweight worker for exploration and discovery",
            )
            .resources(
                ResourceLimits::minimal().max_duration(Duration::from_secs(600)), // 10 min
            )
            .capability("exploration")
            .capability("discovery")
            .termination(TerminationPolicy::WhenTaskComplete)
            .tag("lightweight")
            .tag("exploration"),
        );

        // Benchmarker - runs benchmarks
        self.register(
            TemplateDefinition::new(names::BENCHMARKER, "Runs performance benchmarks")
                .resources(
                    ResourceLimits::heavy().max_duration(Duration::from_secs(7200)), // 2 hours
                )
                .capability("benchmarking")
                .capability("performance")
                .deployment(DeploymentOption::Isolated) // Prefer isolation for consistent results
                .termination(TerminationPolicy::WhenTaskComplete)
                .tag("benchmarking")
                .tag("performance"),
        );

        // Doc generator - generates documentation
        self.register(
            TemplateDefinition::new(names::DOC_GENERATOR, "Generates project documentation")
                .resources(
                    ResourceLimits::standard()
                        .max_cpu(2)
                        .max_memory(4096)
                        .max_duration(Duration::from_secs(1200)), // 20 min
                )
                .capability("documentation")
                .capability("rustdoc")
                .termination(TerminationPolicy::WhenTaskComplete)
                .tag("documentation"),
        );

        // Linter - runs lints and formatting
        self.register(
            TemplateDefinition::new(names::LINTER, "Runs linters and formatters")
                .resources(
                    ResourceLimits::minimal()
                        .max_cpu(2)
                        .max_memory(2048)
                        .max_duration(Duration::from_secs(600)), // 10 min
                )
                .capability("linting")
                .capability("formatting")
                .capability("clippy")
                .capability("rustfmt")
                .termination(TerminationPolicy::WhenTaskComplete)
                .tag("quality")
                .tag("ci"),
        );

        // Watcher - monitors for changes
        self.register(
            TemplateDefinition::new(
                names::WATCHER,
                "Monitors files and triggers actions on change",
            )
            .resources(
                ResourceLimits::minimal().no_duration_limit(), // No time limit
            )
            .capability("watching")
            .capability("monitoring")
            .termination(TerminationPolicy::Manual) // Manual termination
            .tag("monitoring")
            .tag("long-running"),
        );
    }
}

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// Registry trait implementation for unified registry access
impl Registry<TemplateDefinition> for TemplateRegistry {
    fn get(&self, key: &str) -> Option<&TemplateDefinition> {
        self.templates.get(key)
    }

    fn contains(&self, key: &str) -> bool {
        self.templates.contains_key(key)
    }

    fn len(&self) -> usize {
        self.templates.len()
    }
}

/// Quick access to default templates.
pub fn get_template(name: &str) -> Option<&'static TemplateDefinition> {
    default_registry().get(name)
}

/// Check if a template exists in the default registry.
pub fn template_exists(name: &str) -> bool {
    default_registry().contains(name)
}

/// Create a SpawnConfig from a template name and task.
pub fn spawn_from_template(name: &str, task: Task) -> Option<SpawnConfig> {
    get_template(name).map(|t| t.to_spawn_config(task))
}

/// Convenience functions for creating spawn configs from predefined templates.
pub mod quick {
    use super::*;

    /// Create a test runner config.
    pub fn test_runner(crate_name: impl Into<String>) -> SpawnConfig {
        spawn_from_template(names::TEST_RUNNER, Task::test(crate_name))
            .expect("test-runner template should exist")
    }

    /// Create a builder config.
    pub fn builder(target: impl Into<String>, release: bool) -> SpawnConfig {
        spawn_from_template(
            names::BUILDER,
            Task::Build {
                target: target.into(),
                release,
            },
        )
        .expect("builder template should exist")
    }

    /// Create an optimizer config.
    pub fn optimizer(target: impl Into<String>, iterations: u32) -> SpawnConfig {
        spawn_from_template(names::OPTIMIZER, Task::optimize(target, iterations))
            .expect("optimizer template should exist")
    }

    /// Create an analyzer config.
    pub fn analyzer(path: impl Into<PathBuf>, analysis_type: AnalysisType) -> SpawnConfig {
        spawn_from_template(
            names::ANALYZER,
            Task::Analyze {
                path: path.into(),
                analysis_type,
            },
        )
        .expect("analyzer template should exist")
    }

    /// Create a benchmarker config.
    pub fn benchmarker(target: impl Into<String>) -> SpawnConfig {
        spawn_from_template(
            names::BENCHMARKER,
            Task::Command {
                command: "cargo".to_string(),
                args: vec!["bench".to_string(), "--package".to_string(), target.into()],
            },
        )
        .expect("benchmarker template should exist")
    }

    /// Create a linter config.
    pub fn linter(crate_name: impl Into<String>) -> SpawnConfig {
        spawn_from_template(
            names::LINTER,
            Task::Command {
                command: "cargo".to_string(),
                args: vec!["clippy".to_string(), "-p".to_string(), crate_name.into()],
            },
        )
        .expect("linter template should exist")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_registry_defaults() {
        let registry = TemplateRegistry::with_defaults();

        // Check all default templates exist
        assert!(registry.contains(names::TEST_RUNNER));
        assert!(registry.contains(names::BUILDER));
        assert!(registry.contains(names::OPTIMIZER));
        assert!(registry.contains(names::ANALYZER));
        assert!(registry.contains(names::WORKER));
        assert!(registry.contains(names::SCOUT));
        assert!(registry.contains(names::BENCHMARKER));
        assert!(registry.contains(names::DOC_GENERATOR));
        assert!(registry.contains(names::LINTER));
        assert!(registry.contains(names::WATCHER));
    }

    #[test]
    fn test_template_definition() {
        let template = TemplateDefinition::new("test", "A test template")
            .resources(ResourceLimits::minimal())
            .capability("testing")
            .tag("test-tag")
            .allow_spawn();

        assert_eq!(template.name, "test");
        assert_eq!(template.description, "A test template");
        assert!(template.capabilities.contains(&"testing".to_string()));
        assert!(template.tags.contains(&"test-tag".to_string()));
        assert!(template.can_spawn_workers);
    }

    #[test]
    fn test_template_to_spawn_config() {
        let template = get_template(names::TEST_RUNNER).unwrap();
        let config = template.to_spawn_config(Task::test("dashflow-openai"));

        assert!(matches!(config.template, SpawnTemplate::Named(ref n) if n == names::TEST_RUNNER));
        assert!(matches!(
            config.task,
            Task::RunTests { ref crate_name, .. } if crate_name == "dashflow-openai"
        ));
    }

    #[test]
    fn test_spawn_from_template() {
        let config = spawn_from_template(names::BUILDER, Task::build("release")).unwrap();
        assert!(matches!(config.template, SpawnTemplate::Named(ref n) if n == names::BUILDER));
    }

    #[test]
    fn test_quick_helpers() {
        let test_config = quick::test_runner("my-crate");
        assert!(matches!(
            test_config.task,
            Task::RunTests { ref crate_name, .. } if crate_name == "my-crate"
        ));

        let build_config = quick::builder("my-target", true);
        assert!(matches!(
            build_config.task,
            Task::Build { ref target, release } if target == "my-target" && release
        ));

        let opt_config = quick::optimizer("grpo", 100);
        assert!(matches!(
            opt_config.task,
            Task::Optimize { ref target, iterations } if target == "grpo" && iterations == 100
        ));
    }

    #[test]
    fn test_registry_by_tag() {
        let registry = TemplateRegistry::with_defaults();
        let ci_templates = registry.by_tag("ci");

        // TEST_RUNNER, BUILDER, and LINTER should have "ci" tag
        assert!(!ci_templates.is_empty());
        assert!(ci_templates.iter().any(|t| t.name == names::TEST_RUNNER));
        assert!(ci_templates.iter().any(|t| t.name == names::BUILDER));
        assert!(ci_templates.iter().any(|t| t.name == names::LINTER));
    }

    #[test]
    fn test_registry_with_capability() {
        let registry = TemplateRegistry::with_defaults();
        let cargo_templates = registry.with_capability("cargo");

        // TEST_RUNNER and BUILDER should have "cargo" capability
        assert!(!cargo_templates.is_empty());
        assert!(cargo_templates.iter().any(|t| t.name == names::TEST_RUNNER));
        assert!(cargo_templates.iter().any(|t| t.name == names::BUILDER));
    }

    #[test]
    fn test_default_registry_static() {
        // Test that the static default registry works
        assert!(template_exists(names::TEST_RUNNER));
        assert!(get_template(names::OPTIMIZER).is_some());
        assert!(!template_exists("nonexistent"));
    }

    #[test]
    fn test_custom_registry() {
        let mut registry = TemplateRegistry::new();

        // Empty registry has no templates
        assert!(!registry.contains(names::TEST_RUNNER));

        // Register a custom template
        registry.register(
            TemplateDefinition::new("custom-worker", "A custom worker")
                .capability("custom")
                .tag("custom"),
        );

        assert!(registry.contains("custom-worker"));
        assert!(!registry.contains(names::TEST_RUNNER));
    }

    #[test]
    fn test_template_resources() {
        let test_runner = get_template(names::TEST_RUNNER).unwrap();
        assert_eq!(test_runner.default_resources.max_cpu_cores, Some(4));
        assert_eq!(test_runner.default_resources.max_memory_mb, Some(8192));

        let scout = get_template(names::SCOUT).unwrap();
        assert_eq!(scout.default_resources.max_cpu_cores, Some(1)); // Minimal
        assert_eq!(scout.default_resources.max_memory_mb, Some(512));

        let optimizer = get_template(names::OPTIMIZER).unwrap();
        assert_eq!(optimizer.default_resources.max_cpu_cores, Some(8)); // Heavy
        assert!(optimizer.can_spawn_workers);
    }

    #[test]
    fn test_watcher_termination_policy() {
        let watcher = get_template(names::WATCHER).unwrap();
        assert!(matches!(
            watcher.default_termination,
            TerminationPolicy::Manual
        ));
        assert_eq!(watcher.default_resources.max_duration, None); // No time limit
    }
}
