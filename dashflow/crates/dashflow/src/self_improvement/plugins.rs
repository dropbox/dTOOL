// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Plugin architecture for the self-improvement system.
//!
//! This module provides a plugin system that allows external analyzers
//! and planners to be loaded dynamically. Plugins can be:
//! - Built-in (registered at compile time)
//! - Loaded from configuration
//!
//! ## Plugin Registration
//!
//! ```rust,ignore
//! use dashflow::self_improvement::{
//!     PluginManager, PluginInfo, Analyzer, Planner,
//! };
//!
//! let mut manager = PluginManager::new();
//!
//! // Register built-in analyzer
//! manager.register_analyzer(Box::new(MyAnalyzer::new()));
//!
//! // Register built-in planner
//! manager.register_planner(Box::new(MyPlanner::new()));
//!
//! // List plugins
//! for info in manager.list_analyzers() {
//!     println!("Analyzer: {} - {}", info.name, info.description);
//! }
//! ```

use std::collections::HashMap;
use std::fmt;

use super::error::{Result, SelfImprovementError};
use super::traits::{
    AnalysisOutput, Analyzer, AnalyzerContext, AnalyzerRegistry, Planner, PlannerInput,
    PlannerOutput, PlannerRegistry,
};
use crate::introspection::ExecutionTrace;

// =============================================================================
// Plugin Info
// =============================================================================

/// Information about a plugin.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// Unique name of the plugin
    pub name: String,
    /// Description of what the plugin does
    pub description: String,
    /// Version of the plugin
    pub version: String,
    /// Author or maintainer
    pub author: Option<String>,
    /// Category (e.g., "analyzer", "planner")
    pub category: PluginCategory,
    /// Whether the plugin is enabled
    pub enabled: bool,
    /// Priority for ordering (lower = higher priority)
    pub priority: i32,
}

impl PluginInfo {
    /// Creates new plugin info.
    #[must_use]
    pub fn new(name: impl Into<String>, category: PluginCategory) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            version: "1.0.0".to_string(),
            author: None,
            category,
            enabled: true,
            priority: 100,
        }
    }

    /// Sets the description.
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Sets the version.
    #[must_use]
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// Sets the author.
    #[must_use]
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// Sets the priority.
    #[must_use]
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
}

/// Category of plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginCategory {
    /// Analyzer plugin
    Analyzer,
    /// Planner plugin
    Planner,
}

impl fmt::Display for PluginCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Analyzer => write!(f, "analyzer"),
            Self::Planner => write!(f, "planner"),
        }
    }
}

// =============================================================================
// Registered Plugin
// =============================================================================

/// A registered analyzer plugin with metadata.
struct RegisteredAnalyzer {
    info: PluginInfo,
    analyzer: Box<dyn Analyzer>,
}

impl fmt::Debug for RegisteredAnalyzer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegisteredAnalyzer")
            .field("info", &self.info)
            .finish()
    }
}

/// A registered planner plugin with metadata.
struct RegisteredPlanner {
    info: PluginInfo,
    planner: Box<dyn Planner>,
}

impl fmt::Debug for RegisteredPlanner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegisteredPlanner")
            .field("info", &self.info)
            .finish()
    }
}

// =============================================================================
// Plugin Manager
// =============================================================================

/// Manages plugins for the self-improvement system.
#[derive(Default)]
pub struct PluginManager {
    /// Registered analyzer plugins
    analyzers: HashMap<String, RegisteredAnalyzer>,
    /// Registered planner plugins
    planners: HashMap<String, RegisteredPlanner>,
}

impl fmt::Debug for PluginManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PluginManager")
            .field("analyzer_count", &self.analyzers.len())
            .field("planner_count", &self.planners.len())
            .finish()
    }
}

impl PluginManager {
    /// Creates a new empty plugin manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a plugin manager with default built-in plugins.
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut manager = Self::new();
        manager.register_built_ins();
        manager
    }

    /// Registers built-in plugins.
    fn register_built_ins(&mut self) {
        // Built-in analyzers can be registered here
        // For now, we just document that custom implementations
        // of the Analyzer trait can be added

        // Built-in planners can be registered here
    }

    // =========================================================================
    // Analyzer Management
    // =========================================================================

    /// Registers an analyzer plugin.
    pub fn register_analyzer(&mut self, analyzer: Box<dyn Analyzer>) {
        let name = analyzer.name().to_string();
        let info = PluginInfo::new(&name, PluginCategory::Analyzer)
            .with_description(analyzer.description());

        self.analyzers
            .insert(name, RegisteredAnalyzer { info, analyzer });
    }

    /// Registers an analyzer with custom info.
    pub fn register_analyzer_with_info(&mut self, analyzer: Box<dyn Analyzer>, info: PluginInfo) {
        let name = info.name.clone();
        self.analyzers
            .insert(name, RegisteredAnalyzer { info, analyzer });
    }

    /// Unregisters an analyzer by name.
    pub fn unregister_analyzer(&mut self, name: &str) -> bool {
        self.analyzers.remove(name).is_some()
    }

    /// Returns info about a specific analyzer.
    #[must_use]
    pub fn get_analyzer_info(&self, name: &str) -> Option<&PluginInfo> {
        self.analyzers.get(name).map(|r| &r.info)
    }

    /// Lists all registered analyzers.
    #[must_use]
    pub fn list_analyzers(&self) -> Vec<&PluginInfo> {
        let mut infos: Vec<_> = self.analyzers.values().map(|r| &r.info).collect();
        infos.sort_by_key(|i| i.priority);
        infos
    }

    /// Returns the number of registered analyzers.
    #[must_use]
    pub fn analyzer_count(&self) -> usize {
        self.analyzers.len()
    }

    /// Enables or disables an analyzer.
    pub fn set_analyzer_enabled(&mut self, name: &str, enabled: bool) -> bool {
        if let Some(reg) = self.analyzers.get_mut(name) {
            reg.info.enabled = enabled;
            true
        } else {
            false
        }
    }

    /// Runs a specific analyzer.
    pub fn run_analyzer(
        &self,
        name: &str,
        traces: &[ExecutionTrace],
        context: &AnalyzerContext,
    ) -> Result<AnalysisOutput> {
        let registered = self
            .analyzers
            .get(name)
            .ok_or_else(|| SelfImprovementError::Other(format!("Analyzer '{}' not found", name)))?;

        if !registered.info.enabled {
            return Err(SelfImprovementError::Other(format!(
                "Analyzer '{}' is disabled",
                name
            )));
        }

        registered.analyzer.validate(traces)?;
        registered.analyzer.analyze(traces, context)
    }

    /// Runs all enabled analyzers and returns results.
    pub fn run_all_analyzers(
        &self,
        traces: &[ExecutionTrace],
        context: &AnalyzerContext,
    ) -> Vec<AnalyzerResult> {
        let mut results = Vec::new();
        let mut enabled: Vec<_> = self.analyzers.values().filter(|r| r.info.enabled).collect();

        // Sort by priority
        enabled.sort_by_key(|r| r.info.priority);

        for registered in enabled {
            let result = registered
                .analyzer
                .validate(traces)
                .and_then(|()| registered.analyzer.analyze(traces, context));

            results.push(AnalyzerResult {
                name: registered.info.name.clone(),
                result,
            });
        }

        results
    }

    // =========================================================================
    // Planner Management
    // =========================================================================

    /// Registers a planner plugin.
    pub fn register_planner(&mut self, planner: Box<dyn Planner>) {
        let name = planner.name().to_string();
        let info =
            PluginInfo::new(&name, PluginCategory::Planner).with_description(planner.description());

        self.planners
            .insert(name, RegisteredPlanner { info, planner });
    }

    /// Registers a planner with custom info.
    pub fn register_planner_with_info(&mut self, planner: Box<dyn Planner>, info: PluginInfo) {
        let name = info.name.clone();
        self.planners
            .insert(name, RegisteredPlanner { info, planner });
    }

    /// Unregisters a planner by name.
    pub fn unregister_planner(&mut self, name: &str) -> bool {
        self.planners.remove(name).is_some()
    }

    /// Returns info about a specific planner.
    #[must_use]
    pub fn get_planner_info(&self, name: &str) -> Option<&PluginInfo> {
        self.planners.get(name).map(|r| &r.info)
    }

    /// Lists all registered planners.
    #[must_use]
    pub fn list_planners(&self) -> Vec<&PluginInfo> {
        let mut infos: Vec<_> = self.planners.values().map(|r| &r.info).collect();
        infos.sort_by_key(|i| i.priority);
        infos
    }

    /// Returns the number of registered planners.
    #[must_use]
    pub fn planner_count(&self) -> usize {
        self.planners.len()
    }

    /// Enables or disables a planner.
    pub fn set_planner_enabled(&mut self, name: &str, enabled: bool) -> bool {
        if let Some(reg) = self.planners.get_mut(name) {
            reg.info.enabled = enabled;
            true
        } else {
            false
        }
    }

    /// Runs a specific planner.
    pub fn run_planner(&self, name: &str, input: PlannerInput) -> Result<PlannerOutput> {
        let registered = self
            .planners
            .get(name)
            .ok_or_else(|| SelfImprovementError::Other(format!("Planner '{}' not found", name)))?;

        if !registered.info.enabled {
            return Err(SelfImprovementError::Other(format!(
                "Planner '{}' is disabled",
                name
            )));
        }

        registered.planner.validate_input(&input)?;
        registered.planner.generate(input)
    }

    // =========================================================================
    // Registry Conversion
    // =========================================================================

    /// Creates an AnalyzerRegistry from enabled analyzers.
    ///
    /// # Deprecated
    ///
    /// This method returns an **empty** registry because `Box<dyn Analyzer>` cannot
    /// be cloned. Use [`run_all_analyzers`](Self::run_all_analyzers) instead to run
    /// analyzers through the plugin manager.
    ///
    /// M-970: Deprecated to clarify that this returns empty; use run_all_analyzers.
    #[deprecated(
        since = "1.11.20",
        note = "Returns empty registry; use run_all_analyzers() instead"
    )]
    pub fn to_analyzer_registry(&self) -> AnalyzerRegistry {
        AnalyzerRegistry::new()
    }

    /// Creates a PlannerRegistry from enabled planners.
    ///
    /// # Deprecated
    ///
    /// This method returns an **empty** registry because `Box<dyn Planner>` cannot
    /// be cloned. Use [`run_planner`](Self::run_planner) instead to run a planner
    /// through the plugin manager.
    ///
    /// M-970: Deprecated to clarify that this returns empty; use run_planner.
    #[deprecated(
        since = "1.11.20",
        note = "Returns empty registry; use run_planner() instead"
    )]
    pub fn to_planner_registry(&self) -> PlannerRegistry {
        PlannerRegistry::new()
    }
}

// =============================================================================
// Analyzer Result
// =============================================================================

/// Result from running an analyzer.
#[derive(Debug)]
pub struct AnalyzerResult {
    /// Name of the analyzer
    pub name: String,
    /// Result of the analysis
    pub result: Result<AnalysisOutput>,
}

impl AnalyzerResult {
    /// Returns true if the analysis succeeded.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.result.is_ok()
    }

    /// Returns the output if successful.
    #[must_use]
    pub fn output(&self) -> Option<&AnalysisOutput> {
        self.result.as_ref().ok()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::any::Any;

    // Test analyzer implementation
    #[derive(Debug)]
    struct TestAnalyzer {
        name: String,
    }

    impl Analyzer for TestAnalyzer {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "Test analyzer"
        }

        fn analyze(
            &self,
            _traces: &[ExecutionTrace],
            _context: &AnalyzerContext,
        ) -> Result<AnalysisOutput> {
            Ok(AnalysisOutput::CapabilityGaps(vec![]))
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    // Test planner implementation
    #[derive(Debug)]
    struct TestPlanner {
        name: String,
    }

    impl Planner for TestPlanner {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "Test planner"
        }

        fn generate(&self, _input: PlannerInput) -> Result<PlannerOutput> {
            Ok(PlannerOutput::Plans(vec![]))
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    #[test]
    fn test_plugin_info_creation() {
        let info = PluginInfo::new("test-plugin", PluginCategory::Analyzer)
            .with_description("A test plugin")
            .with_version("1.2.3")
            .with_author("Test Author")
            .with_priority(50);

        assert_eq!(info.name, "test-plugin");
        assert_eq!(info.description, "A test plugin");
        assert_eq!(info.version, "1.2.3");
        assert_eq!(info.author, Some("Test Author".to_string()));
        assert_eq!(info.category, PluginCategory::Analyzer);
        assert!(info.enabled);
        assert_eq!(info.priority, 50);
    }

    #[test]
    fn test_plugin_category_display() {
        assert_eq!(PluginCategory::Analyzer.to_string(), "analyzer");
        assert_eq!(PluginCategory::Planner.to_string(), "planner");
    }

    #[test]
    fn test_register_analyzer() {
        let mut manager = PluginManager::new();
        assert_eq!(manager.analyzer_count(), 0);

        manager.register_analyzer(Box::new(TestAnalyzer {
            name: "test-1".to_string(),
        }));
        assert_eq!(manager.analyzer_count(), 1);

        let info = manager.get_analyzer_info("test-1");
        assert!(info.is_some());
        assert_eq!(info.unwrap().name, "test-1");
    }

    #[test]
    fn test_register_planner() {
        let mut manager = PluginManager::new();
        assert_eq!(manager.planner_count(), 0);

        manager.register_planner(Box::new(TestPlanner {
            name: "planner-1".to_string(),
        }));
        assert_eq!(manager.planner_count(), 1);

        let info = manager.get_planner_info("planner-1");
        assert!(info.is_some());
    }

    #[test]
    fn test_unregister_analyzer() {
        let mut manager = PluginManager::new();
        manager.register_analyzer(Box::new(TestAnalyzer {
            name: "test-1".to_string(),
        }));

        assert!(manager.unregister_analyzer("test-1"));
        assert_eq!(manager.analyzer_count(), 0);
        assert!(!manager.unregister_analyzer("test-1")); // Already removed
    }

    #[test]
    fn test_enable_disable_analyzer() {
        let mut manager = PluginManager::new();
        manager.register_analyzer(Box::new(TestAnalyzer {
            name: "test-1".to_string(),
        }));

        // Initially enabled
        let info = manager.get_analyzer_info("test-1").unwrap();
        assert!(info.enabled);

        // Disable
        assert!(manager.set_analyzer_enabled("test-1", false));
        let info = manager.get_analyzer_info("test-1").unwrap();
        assert!(!info.enabled);

        // Enable again
        assert!(manager.set_analyzer_enabled("test-1", true));
        let info = manager.get_analyzer_info("test-1").unwrap();
        assert!(info.enabled);
    }

    #[test]
    fn test_run_analyzer() {
        let mut manager = PluginManager::new();
        manager.register_analyzer(Box::new(TestAnalyzer {
            name: "test-1".to_string(),
        }));

        let trace = ExecutionTrace::builder().build();
        let context = AnalyzerContext::new();

        let result = manager.run_analyzer("test-1", &[trace], &context);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_disabled_analyzer() {
        let mut manager = PluginManager::new();
        manager.register_analyzer(Box::new(TestAnalyzer {
            name: "test-1".to_string(),
        }));
        manager.set_analyzer_enabled("test-1", false);

        let trace = ExecutionTrace::builder().build();
        let context = AnalyzerContext::new();

        let result = manager.run_analyzer("test-1", &[trace], &context);
        assert!(result.is_err());
    }

    #[test]
    fn test_run_all_analyzers() {
        let mut manager = PluginManager::new();
        manager.register_analyzer(Box::new(TestAnalyzer {
            name: "test-1".to_string(),
        }));
        manager.register_analyzer(Box::new(TestAnalyzer {
            name: "test-2".to_string(),
        }));

        let trace = ExecutionTrace::builder().build();
        let context = AnalyzerContext::new();

        let results = manager.run_all_analyzers(&[trace], &context);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.is_success()));
    }

    #[test]
    fn test_list_analyzers_sorted_by_priority() {
        let mut manager = PluginManager::new();

        let analyzer1 = Box::new(TestAnalyzer {
            name: "low-priority".to_string(),
        });
        let info1 = PluginInfo::new("low-priority", PluginCategory::Analyzer).with_priority(100);
        manager.register_analyzer_with_info(analyzer1, info1);

        let analyzer2 = Box::new(TestAnalyzer {
            name: "high-priority".to_string(),
        });
        let info2 = PluginInfo::new("high-priority", PluginCategory::Analyzer).with_priority(10);
        manager.register_analyzer_with_info(analyzer2, info2);

        let infos = manager.list_analyzers();
        assert_eq!(infos.len(), 2);
        assert_eq!(infos[0].name, "high-priority");
        assert_eq!(infos[1].name, "low-priority");
    }

    #[test]
    fn test_run_planner() {
        let mut manager = PluginManager::new();
        manager.register_planner(Box::new(TestPlanner {
            name: "planner-1".to_string(),
        }));

        let input = PlannerInput::Gaps(vec![]);
        let result = manager.run_planner("planner-1", input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_analyzer_result() {
        let success = AnalyzerResult {
            name: "test".to_string(),
            result: Ok(AnalysisOutput::CapabilityGaps(vec![])),
        };
        assert!(success.is_success());
        assert!(success.output().is_some());

        let failure = AnalyzerResult {
            name: "test".to_string(),
            result: Err(SelfImprovementError::Other("error".to_string())),
        };
        assert!(!failure.is_success());
        assert!(failure.output().is_none());
    }
}
