//! Permission checking for plugin actions.
//!
//! This module provides permission gating for plugin operations. Each action
//! a plugin can take is mapped to a required permission, and the checker
//! validates that the plugin has been granted the necessary permissions.
//!
//! ## Permission Model
//!
//! Permissions are granted at plugin load time based on the manifest. Plugins
//! cannot escalate their permissions at runtime. Each host function call
//! checks permissions before executing.
//!
//! ## Dangerous Permissions
//!
//! Some permissions are marked as "dangerous" and require explicit user
//! confirmation before being granted:
//! - `TerminalWrite`: Can inject input as if typed by user
//! - `ClipboardRead`: Can read potentially sensitive clipboard data
//! - `ClipboardWrite`: Can modify clipboard contents

use std::collections::HashSet;

use super::manifest::Permission;
use super::types::{PluginAction, PluginEvent, PluginId};

/// Result of a permission check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionCheckResult {
    /// Permission granted.
    Granted,
    /// Permission denied.
    Denied,
}

impl PermissionCheckResult {
    /// Returns true if permission was granted.
    pub fn is_granted(self) -> bool {
        self == Self::Granted
    }

    /// Returns true if permission was denied.
    pub fn is_denied(self) -> bool {
        self == Self::Denied
    }
}

/// Permission checker for a single plugin.
#[derive(Debug, Clone)]
pub struct PermissionChecker {
    /// Plugin ID (for error reporting).
    plugin_id: PluginId,
    /// Granted permissions.
    permissions: HashSet<Permission>,
}

impl PermissionChecker {
    /// Create a new permission checker for a plugin.
    pub fn new(plugin_id: PluginId, permissions: HashSet<Permission>) -> Self {
        Self {
            plugin_id,
            permissions,
        }
    }

    /// Create a checker with no permissions.
    pub fn empty(plugin_id: PluginId) -> Self {
        Self::new(plugin_id, HashSet::new())
    }

    /// Create a checker with all permissions (for testing).
    pub fn all_permissions(plugin_id: PluginId) -> Self {
        let permissions = [
            Permission::TerminalRead,
            Permission::TerminalWrite,
            Permission::TerminalCommand,
            Permission::Storage,
            Permission::ClipboardRead,
            Permission::ClipboardWrite,
        ]
        .into_iter()
        .collect();
        Self::new(plugin_id, permissions)
    }

    /// Get the plugin ID.
    pub fn plugin_id(&self) -> PluginId {
        self.plugin_id
    }

    /// Check if a specific permission is granted.
    pub fn has_permission(&self, permission: Permission) -> bool {
        self.permissions.contains(&permission)
    }

    /// Check permission and return result.
    pub fn check(&self, permission: Permission) -> PermissionCheckResult {
        if self.has_permission(permission) {
            PermissionCheckResult::Granted
        } else {
            PermissionCheckResult::Denied
        }
    }

    /// Check if the plugin can receive a specific event type.
    ///
    /// Events have read semantics, so they require read permissions.
    pub fn can_receive_event(&self, event: &PluginEvent) -> PermissionCheckResult {
        let required = match event {
            PluginEvent::Output { .. } => Permission::TerminalRead,
            PluginEvent::Key(_) => Permission::TerminalRead,
            PluginEvent::CommandStart { .. } => Permission::TerminalCommand,
            PluginEvent::CommandComplete { .. } => Permission::TerminalCommand,
            PluginEvent::Tick { .. } => {
                // Tick events don't require any specific permission
                return PermissionCheckResult::Granted;
            }
        };
        self.check(required)
    }

    /// Check if the plugin can perform a specific action.
    ///
    /// Actions have write semantics, so they may require write permissions.
    pub fn can_perform_action(&self, action: &PluginAction) -> PermissionCheckResult {
        match action {
            // Continue and Consume don't require special permissions
            PluginAction::Continue | PluginAction::Consume => PermissionCheckResult::Granted,

            // Transform requires write permission
            PluginAction::Transform(_) => self.check(Permission::TerminalWrite),

            // EmitInput requires write permission
            PluginAction::EmitInput(_) => self.check(Permission::TerminalWrite),

            // EmitCommand requires write permission
            PluginAction::EmitCommand { .. } => self.check(Permission::TerminalWrite),

            // Annotate requires read permission (viewing/highlighting)
            PluginAction::Annotate { .. } => self.check(Permission::TerminalRead),
        }
    }

    /// Check if the plugin can use storage.
    pub fn can_use_storage(&self) -> PermissionCheckResult {
        self.check(Permission::Storage)
    }

    /// Check if the plugin can read clipboard.
    pub fn can_read_clipboard(&self) -> PermissionCheckResult {
        self.check(Permission::ClipboardRead)
    }

    /// Check if the plugin can write clipboard.
    pub fn can_write_clipboard(&self) -> PermissionCheckResult {
        self.check(Permission::ClipboardWrite)
    }

    /// Get all granted permissions.
    pub fn granted_permissions(&self) -> &HashSet<Permission> {
        &self.permissions
    }

    /// Get all dangerous permissions that are granted.
    pub fn dangerous_permissions(&self) -> Vec<Permission> {
        self.permissions
            .iter()
            .filter(|p| p.is_dangerous())
            .copied()
            .collect()
    }

    /// Check if any dangerous permissions are granted.
    pub fn has_dangerous_permissions(&self) -> bool {
        self.permissions.iter().any(Permission::is_dangerous)
    }
}

/// Error when a permission check fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionDenied {
    /// The plugin that was denied.
    pub plugin_id: PluginId,
    /// The permission that was required.
    pub required: Permission,
    /// What operation was attempted.
    pub operation: String,
}

impl std::fmt::Display for PermissionDenied {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "plugin {} denied permission '{}' for operation: {}",
            self.plugin_id, self.required, self.operation
        )
    }
}

impl std::error::Error for PermissionDenied {}

/// Helper to check permission and return a Result.
pub fn require_permission(
    checker: &PermissionChecker,
    permission: Permission,
    operation: &str,
) -> Result<(), PermissionDenied> {
    if checker.has_permission(permission) {
        Ok(())
    } else {
        Err(PermissionDenied {
            plugin_id: checker.plugin_id(),
            required: permission,
            operation: operation.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_checker_creation() {
        let perms = [Permission::TerminalRead, Permission::Storage]
            .into_iter()
            .collect();
        let checker = PermissionChecker::new(PluginId(1), perms);

        assert!(checker.has_permission(Permission::TerminalRead));
        assert!(checker.has_permission(Permission::Storage));
        assert!(!checker.has_permission(Permission::TerminalWrite));
    }

    #[test]
    fn test_permission_checker_empty() {
        let checker = PermissionChecker::empty(PluginId(1));
        assert!(!checker.has_permission(Permission::TerminalRead));
        assert!(!checker.has_permission(Permission::Storage));
    }

    #[test]
    fn test_permission_checker_all() {
        let checker = PermissionChecker::all_permissions(PluginId(1));
        assert!(checker.has_permission(Permission::TerminalRead));
        assert!(checker.has_permission(Permission::TerminalWrite));
        assert!(checker.has_permission(Permission::Storage));
        assert!(checker.has_permission(Permission::ClipboardRead));
    }

    #[test]
    fn test_check_result() {
        let result = PermissionCheckResult::Granted;
        assert!(result.is_granted());
        assert!(!result.is_denied());

        let result = PermissionCheckResult::Denied;
        assert!(result.is_denied());
        assert!(!result.is_granted());
    }

    #[test]
    fn test_can_receive_event() {
        let perms = [Permission::TerminalRead].into_iter().collect();
        let checker = PermissionChecker::new(PluginId(1), perms);

        // Can receive output with TerminalRead
        let event = PluginEvent::Output {
            data: vec![],
            in_command: false,
        };
        assert!(checker.can_receive_event(&event).is_granted());

        // Cannot receive command events without TerminalCommand
        let event = PluginEvent::CommandStart {
            command: "ls".to_string(),
            cwd: None,
        };
        assert!(checker.can_receive_event(&event).is_denied());

        // Tick events always allowed
        let event = PluginEvent::Tick { now_ms: 0 };
        assert!(checker.can_receive_event(&event).is_granted());
    }

    #[test]
    fn test_can_perform_action() {
        let perms = [Permission::TerminalRead].into_iter().collect();
        let checker = PermissionChecker::new(PluginId(1), perms);

        // Continue always allowed
        assert!(checker.can_perform_action(&PluginAction::Continue).is_granted());

        // Consume always allowed
        assert!(checker.can_perform_action(&PluginAction::Consume).is_granted());

        // Transform requires TerminalWrite
        assert!(checker
            .can_perform_action(&PluginAction::Transform(vec![]))
            .is_denied());

        // EmitInput requires TerminalWrite
        assert!(checker
            .can_perform_action(&PluginAction::EmitInput(vec![]))
            .is_denied());

        // Annotate requires TerminalRead (which we have)
        let annotate = PluginAction::Annotate {
            start_col: 0,
            end_col: 10,
            start_row: 0,
            end_row: 0,
            style: "highlight".to_string(),
        };
        assert!(checker.can_perform_action(&annotate).is_granted());
    }

    #[test]
    fn test_can_perform_action_with_write() {
        let perms = [Permission::TerminalWrite].into_iter().collect();
        let checker = PermissionChecker::new(PluginId(1), perms);

        // Now Transform should work
        assert!(checker
            .can_perform_action(&PluginAction::Transform(vec![]))
            .is_granted());

        assert!(checker
            .can_perform_action(&PluginAction::EmitInput(vec![]))
            .is_granted());

        let emit = PluginAction::EmitCommand {
            command: "echo".to_string(),
            args: vec!["hello".to_string()],
        };
        assert!(checker.can_perform_action(&emit).is_granted());
    }

    #[test]
    fn test_storage_permission() {
        let checker = PermissionChecker::empty(PluginId(1));
        assert!(checker.can_use_storage().is_denied());

        let perms = [Permission::Storage].into_iter().collect();
        let checker = PermissionChecker::new(PluginId(1), perms);
        assert!(checker.can_use_storage().is_granted());
    }

    #[test]
    fn test_clipboard_permissions() {
        let checker = PermissionChecker::empty(PluginId(1));
        assert!(checker.can_read_clipboard().is_denied());
        assert!(checker.can_write_clipboard().is_denied());

        let perms = [Permission::ClipboardRead].into_iter().collect();
        let checker = PermissionChecker::new(PluginId(1), perms);
        assert!(checker.can_read_clipboard().is_granted());
        assert!(checker.can_write_clipboard().is_denied());
    }

    #[test]
    fn test_dangerous_permissions() {
        let perms = [
            Permission::TerminalRead,
            Permission::TerminalWrite,
            Permission::Storage,
        ]
        .into_iter()
        .collect();
        let checker = PermissionChecker::new(PluginId(1), perms);

        assert!(checker.has_dangerous_permissions());

        let dangerous = checker.dangerous_permissions();
        assert_eq!(dangerous.len(), 1);
        assert!(dangerous.contains(&Permission::TerminalWrite));
    }

    #[test]
    fn test_no_dangerous_permissions() {
        let perms = [Permission::TerminalRead, Permission::Storage]
            .into_iter()
            .collect();
        let checker = PermissionChecker::new(PluginId(1), perms);

        assert!(!checker.has_dangerous_permissions());
        assert!(checker.dangerous_permissions().is_empty());
    }

    #[test]
    fn test_require_permission_ok() {
        let perms = [Permission::Storage].into_iter().collect();
        let checker = PermissionChecker::new(PluginId(1), perms);

        let result = require_permission(&checker, Permission::Storage, "storage_get");
        assert!(result.is_ok());
    }

    #[test]
    fn test_require_permission_denied() {
        let checker = PermissionChecker::empty(PluginId(1));

        let result = require_permission(&checker, Permission::Storage, "storage_get");
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.plugin_id, PluginId(1));
        assert_eq!(err.required, Permission::Storage);
        assert_eq!(err.operation, "storage_get");
    }

    #[test]
    fn test_permission_denied_display() {
        let err = PermissionDenied {
            plugin_id: PluginId(42),
            required: Permission::Storage,
            operation: "storage_set".to_string(),
        };
        assert_eq!(
            format!("{err}"),
            "plugin plugin-42 denied permission 'storage' for operation: storage_set"
        );
    }
}
