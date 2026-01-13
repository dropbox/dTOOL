//! Approval presets module.
//!
//! Provides predefined configurations that pair approval policies with sandbox policies.
//! These presets can be used by CLI tools and TUI to offer users simple choices.

use crate::execpolicy::{ApprovalMode, ExecPolicy};
use crate::{ApprovalPolicy, SandboxPolicy};

/// A preset pairing an approval policy with a sandbox policy.
#[derive(Debug, Clone)]
pub struct ApprovalPreset {
    /// Stable identifier for the preset (e.g., "read-only", "auto").
    pub id: &'static str,
    /// Display label shown in UIs.
    pub label: &'static str,
    /// Short human-readable description.
    pub description: &'static str,
    /// Approval policy for this preset.
    pub approval: ApprovalPolicy,
    /// Sandbox policy for this preset.
    pub sandbox: SandboxPolicy,
}

/// Returns the built-in list of approval presets.
///
/// These presets pair approval and sandbox policies for common use cases.
/// The list is UI-agnostic and can be used by both TUI and CLI.
///
/// # Example
///
/// ```no_run
/// use codex_dashflow_core::approval_presets::builtin_approval_presets;
///
/// let presets = builtin_approval_presets();
/// assert!(!presets.is_empty());
///
/// // Find the read-only preset
/// let read_only = presets.iter().find(|p| p.id == "read-only");
/// assert!(read_only.is_some());
/// ```
pub fn builtin_approval_presets() -> Vec<ApprovalPreset> {
    vec![
        ApprovalPreset {
            id: "read-only",
            label: "Read Only",
            description: "Requires approval to edit files and run commands.",
            approval: ApprovalPolicy::Always,
            sandbox: SandboxPolicy::Native,
        },
        ApprovalPreset {
            id: "auto",
            label: "Agent",
            description: "Read and edit files, and run commands.",
            approval: ApprovalPolicy::OnUnknown,
            sandbox: SandboxPolicy::Native,
        },
        ApprovalPreset {
            id: "full-access",
            label: "Agent (full access)",
            description: "Can edit files and run commands without sandbox. Exercise caution.",
            approval: ApprovalPolicy::Never,
            sandbox: SandboxPolicy::None,
        },
    ]
}

/// Finds a preset by its ID.
///
/// # Arguments
///
/// * `id` - The preset identifier to search for
///
/// # Returns
///
/// The matching preset, or `None` if not found.
///
/// # Example
///
/// ```no_run
/// use codex_dashflow_core::approval_presets::find_preset;
///
/// let preset = find_preset("auto");
/// assert!(preset.is_some());
/// assert_eq!(preset.unwrap().label, "Agent");
/// ```
pub fn find_preset(id: &str) -> Option<ApprovalPreset> {
    builtin_approval_presets().into_iter().find(|p| p.id == id)
}

/// Returns the default preset ID.
///
/// This is the recommended preset for new users.
pub fn default_preset_id() -> &'static str {
    "auto"
}

/// Convert an `ApprovalPolicy` to an `ApprovalMode`.
///
/// This bridges the gap between the high-level preset policy and the
/// execution policy's approval mode.
///
/// # Mapping
///
/// - `ApprovalPolicy::Never` → `ApprovalMode::Never` (auto-approve all)
/// - `ApprovalPolicy::OnUnknown` → `ApprovalMode::OnDangerous` (prompt for dangerous)
/// - `ApprovalPolicy::Always` → `ApprovalMode::Always` (always prompt)
pub fn approval_policy_to_mode(policy: ApprovalPolicy) -> ApprovalMode {
    match policy {
        ApprovalPolicy::Never => ApprovalMode::Never,
        ApprovalPolicy::OnUnknown => ApprovalMode::OnDangerous,
        ApprovalPolicy::Always => ApprovalMode::Always,
    }
}

/// Create an `ExecPolicy` from a preset ID.
///
/// Returns an `ExecPolicy` configured with the approval mode from the preset,
/// plus the default dangerous command patterns.
///
/// # Arguments
///
/// * `preset_id` - The preset identifier (e.g., "auto", "read-only", "full-access")
///
/// # Returns
///
/// An `ExecPolicy` with the appropriate approval mode, or the default policy
/// if the preset is not found.
///
/// # Example
///
/// ```no_run
/// use codex_dashflow_core::approval_presets::exec_policy_from_preset;
///
/// let policy = exec_policy_from_preset("read-only");
/// // Policy will require approval for all tools
/// ```
pub fn exec_policy_from_preset(preset_id: &str) -> ExecPolicy {
    if let Some(preset) = find_preset(preset_id) {
        ExecPolicy::with_dangerous_patterns()
            .with_approval_mode(approval_policy_to_mode(preset.approval))
    } else {
        // Default to the default preset's policy
        if let Some(default) = find_preset(default_preset_id()) {
            ExecPolicy::with_dangerous_patterns()
                .with_approval_mode(approval_policy_to_mode(default.approval))
        } else {
            ExecPolicy::with_dangerous_patterns()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_presets_not_empty() {
        let presets = builtin_approval_presets();
        assert!(!presets.is_empty());
    }

    #[test]
    fn test_builtin_presets_have_read_only() {
        let presets = builtin_approval_presets();
        let read_only = presets.iter().find(|p| p.id == "read-only");
        assert!(read_only.is_some());
        let preset = read_only.unwrap();
        assert_eq!(preset.label, "Read Only");
        assert_eq!(preset.approval, ApprovalPolicy::Always);
    }

    #[test]
    fn test_builtin_presets_have_auto() {
        let presets = builtin_approval_presets();
        let auto = presets.iter().find(|p| p.id == "auto");
        assert!(auto.is_some());
        let preset = auto.unwrap();
        assert_eq!(preset.label, "Agent");
        assert_eq!(preset.approval, ApprovalPolicy::OnUnknown);
    }

    #[test]
    fn test_builtin_presets_have_full_access() {
        let presets = builtin_approval_presets();
        let full = presets.iter().find(|p| p.id == "full-access");
        assert!(full.is_some());
        let preset = full.unwrap();
        assert_eq!(preset.approval, ApprovalPolicy::Never);
        assert_eq!(preset.sandbox, SandboxPolicy::None);
    }

    #[test]
    fn test_find_preset_existing() {
        let preset = find_preset("auto");
        assert!(preset.is_some());
        assert_eq!(preset.unwrap().id, "auto");
    }

    #[test]
    fn test_find_preset_not_found() {
        let preset = find_preset("nonexistent");
        assert!(preset.is_none());
    }

    #[test]
    fn test_default_preset_id() {
        assert_eq!(default_preset_id(), "auto");
        // Verify the default preset exists
        let preset = find_preset(default_preset_id());
        assert!(preset.is_some());
    }

    #[test]
    fn test_preset_unique_ids() {
        let presets = builtin_approval_presets();
        let ids: Vec<_> = presets.iter().map(|p| p.id).collect();
        let unique_count = {
            let mut sorted = ids.clone();
            sorted.sort();
            sorted.dedup();
            sorted.len()
        };
        assert_eq!(ids.len(), unique_count, "Preset IDs must be unique");
    }

    #[test]
    fn test_approval_policy_to_mode_never() {
        let mode = approval_policy_to_mode(ApprovalPolicy::Never);
        assert_eq!(mode, ApprovalMode::Never);
    }

    #[test]
    fn test_approval_policy_to_mode_on_unknown() {
        let mode = approval_policy_to_mode(ApprovalPolicy::OnUnknown);
        assert_eq!(mode, ApprovalMode::OnDangerous);
    }

    #[test]
    fn test_approval_policy_to_mode_always() {
        let mode = approval_policy_to_mode(ApprovalPolicy::Always);
        assert_eq!(mode, ApprovalMode::Always);
    }

    #[test]
    fn test_exec_policy_from_preset_read_only() {
        let policy = exec_policy_from_preset("read-only");
        assert_eq!(policy.approval_mode, ApprovalMode::Always);
    }

    #[test]
    fn test_exec_policy_from_preset_auto() {
        let policy = exec_policy_from_preset("auto");
        assert_eq!(policy.approval_mode, ApprovalMode::OnDangerous);
    }

    #[test]
    fn test_exec_policy_from_preset_full_access() {
        let policy = exec_policy_from_preset("full-access");
        assert_eq!(policy.approval_mode, ApprovalMode::Never);
    }

    #[test]
    fn test_exec_policy_from_preset_unknown_falls_back() {
        // Unknown preset should fall back to default (auto)
        let policy = exec_policy_from_preset("nonexistent");
        assert_eq!(policy.approval_mode, ApprovalMode::OnDangerous);
    }
}
