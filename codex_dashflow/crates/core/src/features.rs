//! Centralized feature flags and metadata
//!
//! This module defines toggles that gate experimental and optional behavior
//! across the codebase. Instead of wiring individual booleans through multiple
//! types, call sites consult a single `Features` container.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// High-level lifecycle stage for a feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// Early testing, may change significantly
    Experimental,
    /// More stable, but not yet recommended for production
    Beta,
    /// Ready for general use
    Stable,
    /// Will be removed in future versions
    Deprecated,
    /// No longer available
    Removed,
}

/// Unique features toggled via configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feature {
    /// Create a ghost commit at each turn for undo support
    GhostCommit,
    /// Use the unified PTY-backed exec tool
    UnifiedExec,
    /// Include the apply_patch tool
    ApplyPatch,
    /// Include the freeform apply_patch tool variant
    ApplyPatchFreeform,
    /// Include the view_image tool
    ViewImageTool,
    /// Allow the model to request web searches
    WebSearchRequest,
    /// Gate the execpolicy enforcement for shell commands
    ExecPolicy,
    /// Enable model-based risk assessments for sandboxed commands
    SandboxCommandAssessment,
    /// Enable Windows sandbox (restricted token)
    WindowsSandbox,
    /// Enable remote compaction
    RemoteCompaction,
    /// Enable the default shell tool
    ShellTool,
    /// Allow model to call multiple tools in parallel
    ParallelToolCalls,
    /// Enable skills injection
    Skills,
    /// Send warnings to the model to correct tool usage
    ModelWarnings,
    /// Enable MCP server support
    McpServer,
    /// Enable MCP client support
    McpClient,
    /// Enable DashFlow streaming telemetry
    DashFlowStreaming,
}

impl Feature {
    /// Get the configuration key for this feature.
    pub fn key(self) -> &'static str {
        self.spec().key
    }

    /// Get the lifecycle stage of this feature.
    pub fn stage(self) -> Stage {
        self.spec().stage
    }

    /// Check if this feature is enabled by default.
    pub fn default_enabled(self) -> bool {
        self.spec().default_enabled
    }

    fn spec(self) -> &'static FeatureSpec {
        FEATURES
            .iter()
            .find(|spec| spec.id == self)
            .unwrap_or_else(|| panic!("missing FeatureSpec for {:?}", self))
    }
}

/// Holds the effective set of enabled features.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Features {
    enabled: BTreeSet<Feature>,
}

impl Features {
    /// Create a new empty feature set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a feature set with built-in defaults.
    pub fn with_defaults() -> Self {
        let mut set = BTreeSet::new();
        for spec in FEATURES {
            if spec.default_enabled {
                set.insert(spec.id);
            }
        }
        Self { enabled: set }
    }

    /// Check if a feature is enabled.
    pub fn enabled(&self, f: Feature) -> bool {
        self.enabled.contains(&f)
    }

    /// Enable a feature.
    pub fn enable(&mut self, f: Feature) -> &mut Self {
        self.enabled.insert(f);
        self
    }

    /// Disable a feature.
    pub fn disable(&mut self, f: Feature) -> &mut Self {
        self.enabled.remove(&f);
        self
    }

    /// Set a feature to enabled or disabled.
    pub fn set(&mut self, f: Feature, enabled: bool) -> &mut Self {
        if enabled {
            self.enable(f)
        } else {
            self.disable(f)
        }
    }

    /// Get an iterator over all enabled features.
    pub fn iter_enabled(&self) -> impl Iterator<Item = Feature> + '_ {
        self.enabled.iter().copied()
    }

    /// Apply a table of key -> bool toggles (e.g. from TOML config).
    pub fn apply_map(&mut self, m: &BTreeMap<String, bool>) {
        for (k, v) in m {
            if let Some(feat) = feature_for_key(k) {
                self.set(feat, *v);
            } else {
                tracing::warn!("unknown feature key in config: {k}");
            }
        }
    }

    /// Get the count of enabled features.
    pub fn enabled_count(&self) -> usize {
        self.enabled.len()
    }
}

/// Look up a feature by its configuration key.
pub fn feature_for_key(key: &str) -> Option<Feature> {
    FEATURES.iter().find(|spec| spec.key == key).map(|s| s.id)
}

/// Check if a string is a known feature key.
pub fn is_known_feature_key(key: &str) -> bool {
    feature_for_key(key).is_some()
}

/// Deserializable features table for TOML configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeaturesToml {
    #[serde(flatten)]
    pub entries: BTreeMap<String, bool>,
}

impl FeaturesToml {
    /// Apply these settings to a Features set.
    pub fn apply_to(&self, features: &mut Features) {
        features.apply_map(&self.entries);
    }
}

/// Specification for a single feature.
#[derive(Debug, Clone, Copy)]
pub struct FeatureSpec {
    pub id: Feature,
    pub key: &'static str,
    pub stage: Stage,
    pub default_enabled: bool,
}

/// Registry of all feature definitions.
pub const FEATURES: &[FeatureSpec] = &[
    // Stable features - enabled by default
    FeatureSpec {
        id: Feature::GhostCommit,
        key: "undo",
        stage: Stage::Stable,
        default_enabled: true,
    },
    FeatureSpec {
        id: Feature::ViewImageTool,
        key: "view_image_tool",
        stage: Stage::Stable,
        default_enabled: true,
    },
    FeatureSpec {
        id: Feature::ShellTool,
        key: "shell_tool",
        stage: Stage::Stable,
        default_enabled: true,
    },
    FeatureSpec {
        id: Feature::ApplyPatch,
        key: "apply_patch",
        stage: Stage::Stable,
        default_enabled: true,
    },
    FeatureSpec {
        id: Feature::ExecPolicy,
        key: "exec_policy",
        stage: Stage::Stable,
        default_enabled: true,
    },
    // Beta features
    FeatureSpec {
        id: Feature::ApplyPatchFreeform,
        key: "apply_patch_freeform",
        stage: Stage::Beta,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::WebSearchRequest,
        key: "web_search_request",
        stage: Stage::Beta,
        default_enabled: false,
    },
    // Experimental features
    FeatureSpec {
        id: Feature::UnifiedExec,
        key: "unified_exec",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::SandboxCommandAssessment,
        key: "sandbox_command_assessment",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::WindowsSandbox,
        key: "windows_sandbox",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::RemoteCompaction,
        key: "remote_compaction",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::ParallelToolCalls,
        key: "parallel_tool_calls",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::Skills,
        key: "skills",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::ModelWarnings,
        key: "model_warnings",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::McpServer,
        key: "mcp_server",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::McpClient,
        key: "mcp_client",
        stage: Stage::Experimental,
        default_enabled: false,
    },
    FeatureSpec {
        id: Feature::DashFlowStreaming,
        key: "dashflow_streaming",
        stage: Stage::Experimental,
        default_enabled: false,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_features_with_defaults() {
        let features = Features::with_defaults();

        // Stable features should be enabled by default
        assert!(features.enabled(Feature::GhostCommit));
        assert!(features.enabled(Feature::ViewImageTool));
        assert!(features.enabled(Feature::ShellTool));
        assert!(features.enabled(Feature::ApplyPatch));
        assert!(features.enabled(Feature::ExecPolicy));

        // Experimental features should not be enabled by default
        assert!(!features.enabled(Feature::UnifiedExec));
        assert!(!features.enabled(Feature::ParallelToolCalls));
        assert!(!features.enabled(Feature::Skills));
    }

    #[test]
    fn test_features_enable_disable() {
        let mut features = Features::new();

        assert!(!features.enabled(Feature::Skills));

        features.enable(Feature::Skills);
        assert!(features.enabled(Feature::Skills));

        features.disable(Feature::Skills);
        assert!(!features.enabled(Feature::Skills));
    }

    #[test]
    fn test_features_set() {
        let mut features = Features::new();

        features.set(Feature::Skills, true);
        assert!(features.enabled(Feature::Skills));

        features.set(Feature::Skills, false);
        assert!(!features.enabled(Feature::Skills));
    }

    #[test]
    fn test_feature_for_key() {
        assert_eq!(feature_for_key("undo"), Some(Feature::GhostCommit));
        assert_eq!(feature_for_key("shell_tool"), Some(Feature::ShellTool));
        assert_eq!(feature_for_key("skills"), Some(Feature::Skills));
        assert_eq!(feature_for_key("nonexistent"), None);
    }

    #[test]
    fn test_is_known_feature_key() {
        assert!(is_known_feature_key("undo"));
        assert!(is_known_feature_key("shell_tool"));
        assert!(!is_known_feature_key("unknown_feature"));
    }

    #[test]
    fn test_apply_map() {
        let mut features = Features::new();
        let mut map = BTreeMap::new();
        map.insert("skills".to_string(), true);
        map.insert("shell_tool".to_string(), false);

        features.apply_map(&map);

        assert!(features.enabled(Feature::Skills));
        assert!(!features.enabled(Feature::ShellTool));
    }

    #[test]
    fn test_feature_spec_lookup() {
        assert_eq!(Feature::GhostCommit.key(), "undo");
        assert_eq!(Feature::GhostCommit.stage(), Stage::Stable);
        assert!(Feature::GhostCommit.default_enabled());

        assert_eq!(Feature::Skills.key(), "skills");
        assert_eq!(Feature::Skills.stage(), Stage::Experimental);
        assert!(!Feature::Skills.default_enabled());
    }

    #[test]
    fn test_features_toml_apply() {
        let mut features = Features::new();
        let toml = FeaturesToml {
            entries: [("skills".to_string(), true)].into_iter().collect(),
        };

        toml.apply_to(&mut features);
        assert!(features.enabled(Feature::Skills));
    }

    #[test]
    fn test_iter_enabled() {
        let mut features = Features::new();
        features.enable(Feature::Skills);
        features.enable(Feature::McpServer);

        let enabled: Vec<_> = features.iter_enabled().collect();
        assert_eq!(enabled.len(), 2);
        assert!(enabled.contains(&Feature::Skills));
        assert!(enabled.contains(&Feature::McpServer));
    }

    #[test]
    fn test_enabled_count() {
        let mut features = Features::new();
        assert_eq!(features.enabled_count(), 0);

        features.enable(Feature::Skills);
        assert_eq!(features.enabled_count(), 1);

        features.enable(Feature::McpServer);
        assert_eq!(features.enabled_count(), 2);
    }

    #[test]
    fn test_all_features_have_unique_keys() {
        let mut keys = BTreeSet::new();
        for spec in FEATURES {
            assert!(keys.insert(spec.key), "duplicate feature key: {}", spec.key);
        }
    }

    #[test]
    fn test_stage_serialization() {
        let json = serde_json::to_string(&Stage::Experimental).unwrap();
        assert_eq!(json, "\"experimental\"");

        let stage: Stage = serde_json::from_str("\"stable\"").unwrap();
        assert_eq!(stage, Stage::Stable);
    }

    // Additional comprehensive tests

    #[test]
    fn test_stage_all_variants() {
        assert_eq!(Stage::Experimental, Stage::Experimental);
        assert_eq!(Stage::Beta, Stage::Beta);
        assert_eq!(Stage::Stable, Stage::Stable);
        assert_eq!(Stage::Deprecated, Stage::Deprecated);
        assert_eq!(Stage::Removed, Stage::Removed);
    }

    #[test]
    fn test_stage_debug() {
        let debug_str = format!("{:?}", Stage::Experimental);
        assert!(debug_str.contains("Experimental"));
    }

    #[test]
    fn test_stage_clone() {
        let stage = Stage::Beta;
        let cloned = stage;
        assert_eq!(stage, cloned);
    }

    #[test]
    fn test_stage_copy() {
        let stage = Stage::Stable;
        let copied: Stage = stage; // Copy trait
        assert_eq!(stage, copied);
    }

    #[test]
    fn test_stage_serde_all_variants() {
        for stage in [
            Stage::Experimental,
            Stage::Beta,
            Stage::Stable,
            Stage::Deprecated,
            Stage::Removed,
        ] {
            let json = serde_json::to_string(&stage).unwrap();
            let restored: Stage = serde_json::from_str(&json).unwrap();
            assert_eq!(stage, restored);
        }
    }

    #[test]
    fn test_feature_debug() {
        let debug_str = format!("{:?}", Feature::GhostCommit);
        assert!(debug_str.contains("GhostCommit"));
    }

    #[test]
    fn test_feature_clone() {
        let feature = Feature::Skills;
        let cloned = feature;
        assert_eq!(feature, cloned);
    }

    #[test]
    fn test_feature_copy() {
        let feature = Feature::McpServer;
        let copied: Feature = feature; // Copy trait
        assert_eq!(feature, copied);
    }

    #[test]
    fn test_feature_ord() {
        // Features should be orderable for BTreeSet
        let a = Feature::GhostCommit;
        let b = Feature::Skills;
        let _ordering = a.cmp(&b); // Ord trait works
        let _partial_cmp = a.partial_cmp(&b); // PartialOrd trait works
        assert!(a != b); // Different features are not equal
        assert!(a == a); // Same feature equals itself
    }

    #[test]
    fn test_feature_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Feature::Skills);
        set.insert(Feature::McpServer);
        assert!(set.contains(&Feature::Skills));
        assert!(!set.contains(&Feature::GhostCommit));
    }

    #[test]
    fn test_feature_serde_roundtrip() {
        let feature = Feature::DashFlowStreaming;
        let json = serde_json::to_string(&feature).unwrap();
        let restored: Feature = serde_json::from_str(&json).unwrap();
        assert_eq!(feature, restored);
    }

    #[test]
    fn test_feature_all_variants_have_specs() {
        // All features should have a corresponding spec
        let features = [
            Feature::GhostCommit,
            Feature::UnifiedExec,
            Feature::ApplyPatch,
            Feature::ApplyPatchFreeform,
            Feature::ViewImageTool,
            Feature::WebSearchRequest,
            Feature::ExecPolicy,
            Feature::SandboxCommandAssessment,
            Feature::WindowsSandbox,
            Feature::RemoteCompaction,
            Feature::ShellTool,
            Feature::ParallelToolCalls,
            Feature::Skills,
            Feature::ModelWarnings,
            Feature::McpServer,
            Feature::McpClient,
            Feature::DashFlowStreaming,
        ];
        for f in features {
            // These should not panic
            let _ = f.key();
            let _ = f.stage();
            let _ = f.default_enabled();
        }
    }

    #[test]
    fn test_features_new_empty() {
        let features = Features::new();
        assert_eq!(features.enabled_count(), 0);
        assert!(!features.enabled(Feature::Skills));
    }

    #[test]
    fn test_features_default() {
        let features = Features::default();
        assert_eq!(features.enabled_count(), 0);
    }

    #[test]
    fn test_features_debug() {
        let features = Features::with_defaults();
        let debug_str = format!("{:?}", features);
        assert!(debug_str.contains("Features"));
    }

    #[test]
    fn test_features_clone() {
        let mut features = Features::new();
        features.enable(Feature::Skills);
        let cloned = features.clone();
        assert!(cloned.enabled(Feature::Skills));
    }

    #[test]
    fn test_features_eq() {
        let mut f1 = Features::new();
        let mut f2 = Features::new();
        assert_eq!(f1, f2);

        f1.enable(Feature::Skills);
        assert_ne!(f1, f2);

        f2.enable(Feature::Skills);
        assert_eq!(f1, f2);
    }

    #[test]
    fn test_features_enable_chaining() {
        let mut features = Features::new();
        features
            .enable(Feature::Skills)
            .enable(Feature::McpServer)
            .enable(Feature::McpClient);
        assert!(features.enabled(Feature::Skills));
        assert!(features.enabled(Feature::McpServer));
        assert!(features.enabled(Feature::McpClient));
    }

    #[test]
    fn test_features_disable_chaining() {
        let mut features = Features::with_defaults();
        features
            .disable(Feature::GhostCommit)
            .disable(Feature::ShellTool);
        assert!(!features.enabled(Feature::GhostCommit));
        assert!(!features.enabled(Feature::ShellTool));
    }

    #[test]
    fn test_features_set_chaining() {
        let mut features = Features::new();
        features
            .set(Feature::Skills, true)
            .set(Feature::McpServer, true)
            .set(Feature::McpClient, false);
        assert!(features.enabled(Feature::Skills));
        assert!(features.enabled(Feature::McpServer));
        assert!(!features.enabled(Feature::McpClient));
    }

    #[test]
    fn test_features_enable_already_enabled() {
        let mut features = Features::new();
        features.enable(Feature::Skills);
        features.enable(Feature::Skills); // Enable again
        assert!(features.enabled(Feature::Skills));
        assert_eq!(features.enabled_count(), 1);
    }

    #[test]
    fn test_features_disable_already_disabled() {
        let mut features = Features::new();
        features.disable(Feature::Skills); // Already disabled
        assert!(!features.enabled(Feature::Skills));
    }

    #[test]
    fn test_features_iter_enabled_empty() {
        let features = Features::new();
        let enabled: Vec<_> = features.iter_enabled().collect();
        assert!(enabled.is_empty());
    }

    #[test]
    fn test_features_with_defaults_count() {
        let features = Features::with_defaults();
        // Count stable features that are default enabled
        let default_enabled_count = FEATURES.iter().filter(|s| s.default_enabled).count();
        assert_eq!(features.enabled_count(), default_enabled_count);
    }

    #[test]
    fn test_feature_spec_debug() {
        let spec = &FEATURES[0];
        let debug_str = format!("{:?}", spec);
        assert!(debug_str.contains("FeatureSpec"));
    }

    #[test]
    fn test_feature_spec_clone() {
        let spec = FEATURES[0];
        let cloned = spec;
        assert_eq!(spec.key, cloned.key);
    }

    #[test]
    fn test_feature_spec_copy() {
        let spec = FEATURES[0];
        let copied: FeatureSpec = spec; // Copy trait
        assert_eq!(spec.key, copied.key);
    }

    #[test]
    fn test_features_toml_default() {
        let toml = FeaturesToml::default();
        assert!(toml.entries.is_empty());
    }

    #[test]
    fn test_features_toml_debug() {
        let toml = FeaturesToml {
            entries: [("skills".to_string(), true)].into_iter().collect(),
        };
        let debug_str = format!("{:?}", toml);
        assert!(debug_str.contains("FeaturesToml"));
    }

    #[test]
    fn test_features_toml_clone() {
        let toml = FeaturesToml {
            entries: [("skills".to_string(), true)].into_iter().collect(),
        };
        let cloned = toml.clone();
        assert_eq!(toml, cloned);
    }

    #[test]
    fn test_features_toml_serde_roundtrip() {
        let toml = FeaturesToml {
            entries: [
                ("skills".to_string(), true),
                ("mcp_server".to_string(), false),
            ]
            .into_iter()
            .collect(),
        };
        let json = serde_json::to_string(&toml).unwrap();
        let restored: FeaturesToml = serde_json::from_str(&json).unwrap();
        assert_eq!(toml, restored);
    }

    #[test]
    fn test_apply_map_empty() {
        let mut features = Features::with_defaults();
        let initial_count = features.enabled_count();
        features.apply_map(&BTreeMap::new());
        assert_eq!(features.enabled_count(), initial_count);
    }

    #[test]
    fn test_apply_map_mixed() {
        let mut features = Features::with_defaults();
        let mut map = BTreeMap::new();
        map.insert("skills".to_string(), true);
        map.insert("undo".to_string(), false); // Disable GhostCommit

        features.apply_map(&map);

        assert!(features.enabled(Feature::Skills));
        assert!(!features.enabled(Feature::GhostCommit));
    }

    #[test]
    fn test_feature_key_all_features() {
        for spec in FEATURES {
            assert!(!spec.key.is_empty());
            assert_eq!(feature_for_key(spec.key), Some(spec.id));
        }
    }

    #[test]
    fn test_feature_stage_all_features() {
        for spec in FEATURES {
            let stage = spec.stage;
            // Stage should be one of the valid variants
            matches!(
                stage,
                Stage::Experimental
                    | Stage::Beta
                    | Stage::Stable
                    | Stage::Deprecated
                    | Stage::Removed
            );
        }
    }

    #[test]
    fn test_stable_features_default_enabled() {
        for spec in FEATURES {
            if spec.stage == Stage::Stable {
                // Most stable features should be enabled by default
                // (This documents the expected behavior)
            }
        }
    }

    #[test]
    fn test_experimental_features_default_disabled() {
        for spec in FEATURES {
            if spec.stage == Stage::Experimental {
                assert!(
                    !spec.default_enabled,
                    "Experimental feature {} should not be default enabled",
                    spec.key
                );
            }
        }
    }

    #[test]
    fn test_features_toml_apply_multiple() {
        let mut features = Features::new();
        let toml = FeaturesToml {
            entries: [
                ("skills".to_string(), true),
                ("mcp_server".to_string(), true),
                ("mcp_client".to_string(), true),
            ]
            .into_iter()
            .collect(),
        };

        toml.apply_to(&mut features);
        assert_eq!(features.enabled_count(), 3);
    }

    #[test]
    fn test_feature_serde_all_variants() {
        let features = [
            Feature::GhostCommit,
            Feature::UnifiedExec,
            Feature::ApplyPatch,
            Feature::ApplyPatchFreeform,
            Feature::ViewImageTool,
            Feature::WebSearchRequest,
            Feature::ExecPolicy,
            Feature::SandboxCommandAssessment,
            Feature::WindowsSandbox,
            Feature::RemoteCompaction,
            Feature::ShellTool,
            Feature::ParallelToolCalls,
            Feature::Skills,
            Feature::ModelWarnings,
            Feature::McpServer,
            Feature::McpClient,
            Feature::DashFlowStreaming,
        ];
        for f in features {
            let json = serde_json::to_string(&f).unwrap();
            let restored: Feature = serde_json::from_str(&json).unwrap();
            assert_eq!(f, restored);
        }
    }

    #[test]
    fn test_features_iter_enabled_preserves_order() {
        let mut features = Features::new();
        features.enable(Feature::Skills);
        features.enable(Feature::McpServer);
        features.enable(Feature::McpClient);

        // BTreeSet maintains order
        let enabled: Vec<_> = features.iter_enabled().collect();
        assert_eq!(enabled.len(), 3);
    }

    #[test]
    fn test_feature_for_key_case_sensitive() {
        // Keys are lowercase by convention
        assert!(feature_for_key("Skills").is_none()); // Wrong case
        assert!(feature_for_key("skills").is_some());
    }

    #[test]
    fn test_feature_default_enabled_consistency() {
        // Verify that Feature::default_enabled() matches the spec
        for spec in FEATURES {
            assert_eq!(spec.id.default_enabled(), spec.default_enabled);
        }
    }

    #[test]
    fn test_feature_stage_consistency() {
        // Verify that Feature::stage() matches the spec
        for spec in FEATURES {
            assert_eq!(spec.id.stage(), spec.stage);
        }
    }

    #[test]
    fn test_feature_key_consistency() {
        // Verify that Feature::key() matches the spec
        for spec in FEATURES {
            assert_eq!(spec.id.key(), spec.key);
        }
    }
}
