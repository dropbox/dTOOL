//! Plugin manifest parsing and validation.
//!
//! Plugin manifests are TOML files that describe the plugin's metadata,
//! required permissions, and configuration.

use std::collections::HashSet;
use std::fmt;
use std::path::Path;

/// Minimum dterm version string (semver).
pub const MIN_SUPPORTED_VERSION: &str = "0.9.0";

/// Plugin manifest parsed from `plugin.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginManifest {
    /// Plugin name (must be unique).
    pub name: String,
    /// Plugin version (semver).
    pub version: String,
    /// WASM entry point file.
    pub entry: String,
    /// Minimum dterm version required.
    pub min_dterm_version: String,
    /// Permissions requested by the plugin.
    pub permissions: HashSet<Permission>,
    /// Human-readable description.
    pub description: Option<String>,
    /// Author information.
    pub author: Option<String>,
    /// Homepage/repository URL.
    pub homepage: Option<String>,
}

impl PluginManifest {
    /// Create a new manifest with required fields.
    pub fn new(name: impl Into<String>, version: impl Into<String>, entry: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            entry: entry.into(),
            min_dterm_version: MIN_SUPPORTED_VERSION.to_string(),
            permissions: HashSet::new(),
            description: None,
            author: None,
            homepage: None,
        }
    }

    /// Add a permission to the manifest.
    #[must_use]
    pub fn with_permission(mut self, permission: Permission) -> Self {
        self.permissions.insert(permission);
        self
    }

    /// Add multiple permissions.
    #[must_use]
    pub fn with_permissions(mut self, permissions: impl IntoIterator<Item = Permission>) -> Self {
        self.permissions.extend(permissions);
        self
    }

    /// Set description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set author.
    #[must_use]
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// Check if plugin requests a specific permission.
    pub fn has_permission(&self, permission: Permission) -> bool {
        self.permissions.contains(&permission)
    }
}

/// Permissions that plugins can request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    /// Read terminal output and state.
    TerminalRead,
    /// Write/transform input to terminal.
    TerminalWrite,
    /// Observe command blocks and metadata.
    TerminalCommand,
    /// Use host-provided key/value storage.
    Storage,
    /// Read from clipboard.
    ClipboardRead,
    /// Write to clipboard.
    ClipboardWrite,
}

impl Permission {
    /// Parse permission from string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "terminal.read" => Some(Self::TerminalRead),
            "terminal.write" => Some(Self::TerminalWrite),
            "terminal.command" => Some(Self::TerminalCommand),
            "storage" => Some(Self::Storage),
            "clipboard.read" => Some(Self::ClipboardRead),
            "clipboard.write" => Some(Self::ClipboardWrite),
            _ => None,
        }
    }

    /// Convert permission to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TerminalRead => "terminal.read",
            Self::TerminalWrite => "terminal.write",
            Self::TerminalCommand => "terminal.command",
            Self::Storage => "storage",
            Self::ClipboardRead => "clipboard.read",
            Self::ClipboardWrite => "clipboard.write",
        }
    }

    /// Check if this permission is dangerous and requires explicit user consent.
    pub fn is_dangerous(&self) -> bool {
        matches!(
            self,
            Self::TerminalWrite | Self::ClipboardWrite | Self::ClipboardRead
        )
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Errors from manifest parsing and validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// File not found.
    FileNotFound(String),
    /// Invalid TOML syntax.
    InvalidToml(String),
    /// Missing required field.
    MissingField(&'static str),
    /// Invalid field value.
    InvalidField {
        /// Field name.
        field: &'static str,
        /// Error message.
        message: String,
    },
    /// Unknown permission.
    UnknownPermission(String),
    /// Version requirement not satisfied.
    VersionMismatch {
        /// Required version.
        required: String,
        /// Current version.
        current: String,
    },
    /// Entry file not found.
    EntryNotFound(String),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FileNotFound(path) => write!(f, "manifest not found: {path}"),
            Self::InvalidToml(msg) => write!(f, "invalid TOML: {msg}"),
            Self::MissingField(field) => write!(f, "missing required field: {field}"),
            Self::InvalidField { field, message } => {
                write!(f, "invalid field '{field}': {message}")
            }
            Self::UnknownPermission(perm) => write!(f, "unknown permission: {perm}"),
            Self::VersionMismatch { required, current } => {
                write!(f, "version mismatch: requires {required}, have {current}")
            }
            Self::EntryNotFound(path) => write!(f, "entry file not found: {path}"),
        }
    }
}

impl std::error::Error for ManifestError {}

/// Parse a plugin manifest from TOML string.
pub fn parse_manifest(toml_content: &str) -> Result<PluginManifest, ManifestError> {
    // Simple TOML parser for manifest format.
    // We avoid pulling in the full `toml` crate to keep dependencies minimal.

    let mut name = None;
    let mut version = None;
    let mut entry = None;
    let mut min_dterm_version = None;
    let mut permissions = HashSet::new();
    let mut description = None;
    let mut author = None;
    let mut homepage = None;

    for line in toml_content.lines() {
        let line = line.trim();

        // Skip comments and empty lines
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Parse key = value
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            // Handle string values (remove quotes)
            let string_value = if value.starts_with('"') && value.ends_with('"') {
                &value[1..value.len() - 1]
            } else {
                value
            };

            match key {
                "name" => name = Some(string_value.to_string()),
                "version" => version = Some(string_value.to_string()),
                "entry" => entry = Some(string_value.to_string()),
                "min_dterm_version" => min_dterm_version = Some(string_value.to_string()),
                "description" => description = Some(string_value.to_string()),
                "author" => author = Some(string_value.to_string()),
                "homepage" => homepage = Some(string_value.to_string()),
                "permissions" => {
                    // Parse array: ["terminal.read", "storage"]
                    if value.starts_with('[') && value.ends_with(']') {
                        let inner = &value[1..value.len() - 1];
                        for perm_str in inner.split(',') {
                            let perm_str = perm_str.trim();
                            if perm_str.is_empty() {
                                continue;
                            }
                            // Remove quotes
                            let perm_str = if perm_str.starts_with('"') && perm_str.ends_with('"') {
                                &perm_str[1..perm_str.len() - 1]
                            } else {
                                perm_str
                            };

                            let perm = Permission::from_str(perm_str)
                                .ok_or_else(|| ManifestError::UnknownPermission(perm_str.to_string()))?;
                            permissions.insert(perm);
                        }
                    }
                }
                _ => {} // Ignore unknown fields for forward compatibility
            }
        }
    }

    // Validate required fields
    let name = name.ok_or(ManifestError::MissingField("name"))?;
    let version = version.ok_or(ManifestError::MissingField("version"))?;
    let entry = entry.ok_or(ManifestError::MissingField("entry"))?;

    // Validate name format (alphanumeric, hyphens, underscores)
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(ManifestError::InvalidField {
            field: "name",
            message: "must contain only alphanumeric characters, hyphens, and underscores".to_string(),
        });
    }

    // Validate name length
    if name.is_empty() || name.len() > 64 {
        return Err(ManifestError::InvalidField {
            field: "name",
            message: "must be 1-64 characters".to_string(),
        });
    }

    Ok(PluginManifest {
        name,
        version,
        entry,
        min_dterm_version: min_dterm_version.unwrap_or_else(|| MIN_SUPPORTED_VERSION.to_string()),
        permissions,
        description,
        author,
        homepage,
    })
}

/// Validate a manifest against system requirements.
pub fn validate_manifest(manifest: &PluginManifest, plugin_dir: &Path) -> Result<(), ManifestError> {
    // Check entry file exists
    let entry_path = plugin_dir.join(&manifest.entry);
    if !entry_path.exists() {
        return Err(ManifestError::EntryNotFound(manifest.entry.clone()));
    }

    // Validate entry file extension
    let entry_path_check = Path::new(&manifest.entry);
    let has_wasm_ext = entry_path_check
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("wasm"));
    if !has_wasm_ext {
        return Err(ManifestError::InvalidField {
            field: "entry",
            message: "must be a .wasm file".to_string(),
        });
    }

    // Version check is deferred to runtime (requires semver parsing)

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_parse_minimal_manifest() {
        let toml = r#"
name = "test-plugin"
version = "0.1.0"
entry = "plugin.wasm"
"#;
        let manifest = parse_manifest(toml).unwrap();
        assert_eq!(manifest.name, "test-plugin");
        assert_eq!(manifest.version, "0.1.0");
        assert_eq!(manifest.entry, "plugin.wasm");
        assert!(manifest.permissions.is_empty());
    }

    #[test]
    fn test_parse_full_manifest() {
        let toml = r#"
name = "example-plugin"
version = "0.1.0"
entry = "plugin.wasm"
min_dterm_version = "0.9.0"
permissions = ["terminal.read", "terminal.write", "storage"]
description = "An example plugin"
author = "Test Author"
homepage = "https://example.com"
"#;
        let manifest = parse_manifest(toml).unwrap();
        assert_eq!(manifest.name, "example-plugin");
        assert_eq!(manifest.permissions.len(), 3);
        assert!(manifest.has_permission(Permission::TerminalRead));
        assert!(manifest.has_permission(Permission::TerminalWrite));
        assert!(manifest.has_permission(Permission::Storage));
        assert_eq!(manifest.description.as_deref(), Some("An example plugin"));
    }

    #[test]
    fn test_missing_name() {
        let toml = r#"
version = "0.1.0"
entry = "plugin.wasm"
"#;
        let err = parse_manifest(toml).unwrap_err();
        assert!(matches!(err, ManifestError::MissingField("name")));
    }

    #[test]
    fn test_invalid_permission() {
        let toml = r#"
name = "test"
version = "0.1.0"
entry = "plugin.wasm"
permissions = ["unknown.perm"]
"#;
        let err = parse_manifest(toml).unwrap_err();
        assert!(matches!(err, ManifestError::UnknownPermission(_)));
    }

    #[test]
    fn test_invalid_name_format() {
        let toml = r#"
name = "test plugin with spaces"
version = "0.1.0"
entry = "plugin.wasm"
"#;
        let err = parse_manifest(toml).unwrap_err();
        assert!(matches!(err, ManifestError::InvalidField { field: "name", .. }));
    }

    #[test]
    fn test_permission_from_str() {
        assert_eq!(Permission::from_str("terminal.read"), Some(Permission::TerminalRead));
        assert_eq!(Permission::from_str("terminal.write"), Some(Permission::TerminalWrite));
        assert_eq!(Permission::from_str("storage"), Some(Permission::Storage));
        assert_eq!(Permission::from_str("invalid"), None);
    }

    #[test]
    fn test_permission_display() {
        assert_eq!(format!("{}", Permission::TerminalRead), "terminal.read");
        assert_eq!(format!("{}", Permission::Storage), "storage");
    }

    #[test]
    fn test_dangerous_permissions() {
        assert!(Permission::TerminalWrite.is_dangerous());
        assert!(Permission::ClipboardWrite.is_dangerous());
        assert!(Permission::ClipboardRead.is_dangerous());
        assert!(!Permission::TerminalRead.is_dangerous());
        assert!(!Permission::Storage.is_dangerous());
    }

    #[test]
    fn test_validate_manifest() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path();

        // Create plugin.wasm file
        fs::write(plugin_dir.join("plugin.wasm"), b"fake wasm").unwrap();

        let manifest = PluginManifest::new("test", "0.1.0", "plugin.wasm");
        assert!(validate_manifest(&manifest, plugin_dir).is_ok());
    }

    #[test]
    fn test_validate_missing_entry() {
        let temp_dir = TempDir::new().unwrap();
        let manifest = PluginManifest::new("test", "0.1.0", "missing.wasm");
        let err = validate_manifest(&manifest, temp_dir.path()).unwrap_err();
        assert!(matches!(err, ManifestError::EntryNotFound(_)));
    }

    #[test]
    fn test_validate_wrong_extension() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("plugin.js"), b"not wasm").unwrap();

        let manifest = PluginManifest::new("test", "0.1.0", "plugin.js");
        let err = validate_manifest(&manifest, temp_dir.path()).unwrap_err();
        assert!(matches!(err, ManifestError::InvalidField { field: "entry", .. }));
    }

    #[test]
    fn test_manifest_builder() {
        let manifest = PluginManifest::new("my-plugin", "1.0.0", "plugin.wasm")
            .with_permission(Permission::TerminalRead)
            .with_permission(Permission::Storage)
            .with_description("A useful plugin")
            .with_author("Author Name");

        assert_eq!(manifest.name, "my-plugin");
        assert!(manifest.has_permission(Permission::TerminalRead));
        assert!(manifest.has_permission(Permission::Storage));
        assert!(!manifest.has_permission(Permission::TerminalWrite));
        assert_eq!(manifest.description.as_deref(), Some("A useful plugin"));
        assert_eq!(manifest.author.as_deref(), Some("Author Name"));
    }
}
