// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Executor introspection types
//!
//! This module contains introspection types that are returned by compiled graphs
//! to provide AI agents with self-awareness capabilities.

// ============================================================================
// Unified Introspection Types
// ============================================================================

/// Unified introspection data for AI self-awareness
///
/// This struct combines all introspection capabilities into a single package
/// that AI agents can use to understand themselves completely.
///
/// **Default-enabled:** Returned by `CompiledGraph::introspect()` with zero configuration.
///
/// # Example
///
/// ```rust,ignore
/// let knowledge = app.introspect();
///
/// // Graph structure
/// println!("Nodes: {}", knowledge.manifest.nodes.len());
///
/// // Platform capabilities
/// println!("Features: {}", knowledge.platform.features.len());
///
/// // App architecture
/// println!("Using checkpointing: {}", knowledge.architecture.has_checkpointer);
/// ```
#[derive(Debug, Clone)]
pub struct GraphIntrospection {
    /// Graph structure and metadata
    pub manifest: crate::introspection::GraphManifest,
    /// DashFlow platform knowledge
    pub platform: crate::platform_registry::PlatformRegistry,
    /// Application architecture analysis
    pub architecture: crate::platform_registry::AppArchitecture,
    /// Runtime capabilities
    pub capabilities: crate::introspection::CapabilityManifest,
}

impl GraphIntrospection {
    /// Convert to JSON for AI consumption
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> std::result::Result<String, serde_json::Error> {
        // Manually construct JSON since fields have different serialization
        let manifest_json = self.manifest.to_json()?;
        let platform_json = self.platform.to_json()?;
        let architecture_json = self.architecture.to_json()?;
        let capabilities_json = self.capabilities.to_json()?;

        Ok(format!(
            r#"{{"manifest":{},"platform":{},"architecture":{},"capabilities":{}}}"#,
            manifest_json, platform_json, architecture_json, capabilities_json
        ))
    }
}

// ============================================================================
// Unified Introspection
// ============================================================================

/// Unified introspection combining all three levels: Platform, App, and Live.
///
/// This struct provides a complete view of DashFlow introspection capabilities:
/// - **Platform**: DashFlow framework capabilities (shared by all apps)
/// - **App**: Application-specific configuration (per compiled graph)
/// - **Live**: Runtime execution state (per execution instance)
///
/// # Example
///
/// ```rust,ignore
/// let compiled = graph.compile()?;
///
/// // Get unified introspection
/// let unified = compiled.unified_introspection();
///
/// // Access all three levels
/// println!("DashFlow version: {}", unified.platform.version_info().version);
/// println!("Graph name: {}", unified.app.manifest.graph_name);
/// println!("Active executions: {}", unified.live.len());
/// ```
#[derive(Debug, Clone)]
pub struct UnifiedIntrospection {
    /// Platform-level introspection: DashFlow framework capabilities.
    /// Shared by ALL DashFlow applications.
    pub platform: crate::platform_introspection::PlatformIntrospection,

    /// App-level introspection: Application-specific configuration.
    /// Contains manifest, architecture, and capabilities for this compiled graph.
    pub app: GraphIntrospection,

    /// Live execution summaries: Currently active and recently completed executions.
    /// Empty if no execution tracker is attached.
    pub live: Vec<crate::live_introspection::ExecutionSummary>,
}

impl UnifiedIntrospection {
    /// Convert to JSON for AI consumption.
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails.
    pub fn to_json(&self) -> std::result::Result<String, serde_json::Error> {
        let platform_json = self.platform.to_json();
        let app_json = self.app.to_json()?;
        let live_json = serde_json::to_string(&self.live)?;

        Ok(format!(
            r#"{{"platform":{},"app":{},"live":{}}}"#,
            platform_json, app_json, live_json
        ))
    }

    /// Get the number of active executions.
    #[must_use]
    pub fn active_execution_count(&self) -> usize {
        self.live
            .iter()
            .filter(|e| {
                matches!(
                    e.status,
                    crate::live_introspection::LiveExecutionStatus::Running
                        | crate::live_introspection::LiveExecutionStatus::Paused
                        | crate::live_introspection::LiveExecutionStatus::WaitingForInput
                )
            })
            .count()
    }

    /// Check if any executions are currently running.
    #[must_use]
    pub fn has_active_executions(&self) -> bool {
        self.active_execution_count() > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unified_introspection_to_json_is_valid_json() {
        let manifest = crate::introspection::GraphManifest::builder()
            .entry_point("start")
            .build()
            .expect("entry_point is required");

        let app = GraphIntrospection {
            manifest,
            platform: crate::platform_registry::PlatformRegistry::discover(),
            architecture: crate::platform_registry::AppArchitecture::builder().build(),
            capabilities: crate::introspection::CapabilityManifest::new(),
        };

        let unified = UnifiedIntrospection {
            platform: crate::platform_introspection::PlatformIntrospection::default(),
            app,
            live: vec![crate::live_introspection::ExecutionSummary {
                execution_id: "exec_1".to_string(),
                graph_name: "graph".to_string(),
                started_at: "2025-01-01T00:00:00Z".to_string(),
                current_node: "start".to_string(),
                iteration: 0,
                status: crate::live_introspection::LiveExecutionStatus::Running,
            }],
        };

        let json = unified.to_json().expect("unified introspection should serialize");
        let value: serde_json::Value = serde_json::from_str(&json).expect("json should parse");

        assert!(value.get("platform").is_some());
        assert!(value.get("app").is_some());
        assert!(value.get("live").is_some());
    }
}
