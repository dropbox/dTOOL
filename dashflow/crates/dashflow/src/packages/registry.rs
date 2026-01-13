// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Local package registry implementation.
//!
//! The local registry stores packages in `~/.dashflow/packages/` with an index file
//! for fast lookups. This is the primary registry for development and testing.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::config::{RequiredSignatures, TrustConfig};
use super::manifest::PackageManifest;
use super::types::{PackageId, PackageType, Signature, TrustLevel, Version, VersionReq};
use crate::registry_trait::Registry;

/// Result type for registry operations.
pub type RegistryResult<T> = Result<T, RegistryError>;

/// Errors that can occur during registry operations.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum RegistryError {
    /// Package not found
    PackageNotFound(String),
    /// Version not found
    VersionNotFound {
        /// Package identifier that was queried.
        package: String,
        /// Version string that was not found.
        version: String,
    },
    /// Invalid manifest
    InvalidManifest(String),
    /// IO error
    Io(String),
    /// Package already exists
    PackageExists(String),
    /// Index corrupted
    IndexCorrupted(String),
    /// Invalid package path
    InvalidPath(String),
    /// Trust requirement not met: signature required but not provided
    SignatureRequired {
        /// Package identifier that required a signature.
        package: String,
    },
    /// Trust requirement not met: specific key signature required
    SignatureKeyRequired {
        /// Package identifier that required a specific key signature.
        package: String,
        /// Key ID that was required for signing.
        required_key: String,
    },
    /// Trust requirement not met: official signature required
    OfficialSignatureRequired {
        /// Package identifier that required an official signature.
        package: String,
    },
    /// Trust requirement not met: insufficient trust level
    InsufficientTrustLevel {
        /// Package identifier that failed trust verification.
        package: String,
        /// Required trust level string.
        required: String,
        /// Actual trust level string of the package.
        actual: String,
    },
    /// Trust requirement not met: package has security vulnerabilities
    VulnerablePackage {
        /// Package identifier with vulnerabilities.
        package: String,
        /// Number of security advisories affecting this package.
        advisory_count: usize,
    },
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PackageNotFound(id) => write!(f, "Package not found: {}", id),
            Self::VersionNotFound { package, version } => {
                write!(f, "Version {} not found for package {}", version, package)
            }
            Self::InvalidManifest(msg) => write!(f, "Invalid manifest: {}", msg),
            Self::Io(msg) => write!(f, "IO error: {}", msg),
            Self::PackageExists(id) => write!(f, "Package already exists: {}", id),
            Self::IndexCorrupted(msg) => write!(f, "Index corrupted: {}", msg),
            Self::InvalidPath(msg) => write!(f, "Invalid path: {}", msg),
            Self::SignatureRequired { package } => {
                write!(
                    f,
                    "Signature required for package '{}' but none provided",
                    package
                )
            }
            Self::SignatureKeyRequired {
                package,
                required_key,
            } => {
                write!(
                    f,
                    "Package '{}' requires signature from key '{}' but not found",
                    package, required_key
                )
            }
            Self::OfficialSignatureRequired { package } => {
                write!(
                    f,
                    "Package '{}' requires official DashFlow signature but none found",
                    package
                )
            }
            Self::InsufficientTrustLevel {
                package,
                required,
                actual,
            } => {
                write!(
                    f,
                    "Package '{}' has trust level '{}' but '{}' required",
                    package, actual, required
                )
            }
            Self::VulnerablePackage {
                package,
                advisory_count,
            } => {
                write!(
                    f,
                    "Package '{}' has {} security advisory(ies) and reject_vulnerable is enabled",
                    package, advisory_count
                )
            }
        }
    }
}

impl std::error::Error for RegistryError {}

/// Local package registry.
///
/// Stores packages in a directory structure:
/// ```text
/// ~/.dashflow/packages/
/// ├── index.json           # Package index for fast lookups
/// ├── dashflow/
/// │   └── sentiment-analysis/
/// │       ├── 1.0.0/
/// │       │   ├── dashflow.toml  # Package manifest
/// │       │   └── ...            # Package contents
/// │       └── 1.2.0/
/// │           └── ...
/// └── community/
///     └── my-package/
///         └── ...
/// ```
#[derive(Debug)]
pub struct LocalRegistry {
    /// Root directory of the registry
    root: PathBuf,
    /// In-memory package index
    index: PackageIndex,
}

impl LocalRegistry {
    /// Create a new local registry at the given path.
    ///
    /// Creates the directory if it doesn't exist.
    pub fn new(root: impl Into<PathBuf>) -> RegistryResult<Self> {
        let root = root.into();
        let root = expand_tilde(&root);

        // Create directory if needed
        if !root.exists() {
            std::fs::create_dir_all(&root).map_err(|e| {
                RegistryError::Io(format!("Failed to create registry directory: {}", e))
            })?;
        }

        // Load or create index
        let index_path = root.join("index.json");
        let index = if index_path.exists() {
            let content = std::fs::read_to_string(&index_path)
                .map_err(|e| RegistryError::Io(format!("Failed to read index: {}", e)))?;
            serde_json::from_str(&content).map_err(|e| {
                RegistryError::IndexCorrupted(format!("Failed to parse index: {}", e))
            })?
        } else {
            PackageIndex::default()
        };

        Ok(Self { root, index })
    }

    /// Create a new local registry using the default path (~/.dashflow/packages).
    pub fn default_path() -> RegistryResult<Self> {
        Self::new("~/.dashflow/packages")
    }

    /// Get the root directory of the registry.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Get the package index.
    pub fn index(&self) -> &PackageIndex {
        &self.index
    }

    /// Save the index to disk.
    pub fn save_index(&self) -> RegistryResult<()> {
        let index_path = self.root.join("index.json");
        let content = serde_json::to_string_pretty(&self.index)
            .map_err(|e| RegistryError::Io(format!("Failed to serialize index: {}", e)))?;
        std::fs::write(&index_path, content)
            .map_err(|e| RegistryError::Io(format!("Failed to write index: {}", e)))?;
        Ok(())
    }

    /// Search for packages matching a query.
    ///
    /// Searches package names, descriptions, and keywords.
    pub fn search(&self, query: &str) -> Vec<&PackageEntry> {
        let query_lower = query.to_lowercase();
        self.index
            .packages
            .values()
            .filter(|entry| {
                entry.name.to_lowercase().contains(&query_lower)
                    || entry.id.to_string().to_lowercase().contains(&query_lower)
                    || entry.description.to_lowercase().contains(&query_lower)
                    || entry
                        .keywords
                        .iter()
                        .any(|k| k.to_lowercase().contains(&query_lower))
            })
            .collect()
    }

    /// List all packages of a specific type.
    pub fn list_by_type(&self, package_type: PackageType) -> Vec<&PackageEntry> {
        self.index
            .packages
            .values()
            .filter(|entry| entry.package_type == package_type)
            .collect()
    }

    /// Get a package entry by ID.
    pub fn get(&self, id: &PackageId) -> Option<&PackageEntry> {
        self.index.packages.get(&id.to_string())
    }

    /// Check if a package exists.
    pub fn exists(&self, id: &PackageId) -> bool {
        self.index.packages.contains_key(&id.to_string())
    }

    /// Check if a specific version exists.
    pub fn version_exists(&self, id: &PackageId, version: &Version) -> bool {
        self.get(id)
            .map(|entry| entry.versions.iter().any(|v| v == version))
            .unwrap_or(false)
    }

    /// Get the latest version of a package.
    pub fn latest_version(&self, id: &PackageId) -> Option<&Version> {
        self.get(id).and_then(|entry| entry.versions.iter().max())
    }

    /// Find versions matching a requirement.
    pub fn find_matching_versions(&self, id: &PackageId, req: &VersionReq) -> Vec<&Version> {
        self.get(id)
            .map(|entry| entry.versions.iter().filter(|v| req.matches(v)).collect())
            .unwrap_or_default()
    }

    /// Get the best matching version for a requirement.
    pub fn best_matching_version(&self, id: &PackageId, req: &VersionReq) -> Option<&Version> {
        self.find_matching_versions(id, req).into_iter().max()
    }

    /// Get the path to a package directory.
    pub fn package_path(&self, id: &PackageId) -> PathBuf {
        self.root.join(id.namespace()).join(id.name())
    }

    /// Get the path to a specific version directory.
    pub fn version_path(&self, id: &PackageId, version: &Version) -> PathBuf {
        self.package_path(id).join(version.to_string())
    }

    /// Get the path to a manifest file.
    pub fn manifest_path(&self, id: &PackageId, version: &Version) -> PathBuf {
        self.version_path(id, version).join("dashflow.toml")
    }

    /// Read a package manifest from disk.
    pub fn read_manifest(
        &self,
        id: &PackageId,
        version: &Version,
    ) -> RegistryResult<PackageManifest> {
        let path = self.manifest_path(id, version);
        if !path.exists() {
            return Err(RegistryError::VersionNotFound {
                package: id.to_string(),
                version: version.to_string(),
            });
        }

        let content = std::fs::read_to_string(&path)
            .map_err(|e| RegistryError::Io(format!("Failed to read manifest: {}", e)))?;

        // Try TOML first, then JSON
        if path.extension().is_some_and(|e| e == "toml") {
            toml::from_str(&content)
                .map_err(|e| RegistryError::InvalidManifest(format!("TOML parse error: {}", e)))
        } else {
            serde_json::from_str(&content)
                .map_err(|e| RegistryError::InvalidManifest(format!("JSON parse error: {}", e)))
        }
    }

    /// Add a package to the registry.
    ///
    /// This creates the directory structure and writes the manifest.
    pub fn add(&mut self, manifest: &PackageManifest) -> RegistryResult<()> {
        let id = &manifest.id;
        let version = &manifest.version;

        // Check if this version already exists
        if self.version_exists(id, version) {
            return Err(RegistryError::PackageExists(format!("{}@{}", id, version)));
        }

        // Create directory structure
        let version_dir = self.version_path(id, version);
        std::fs::create_dir_all(&version_dir)
            .map_err(|e| RegistryError::Io(format!("Failed to create package directory: {}", e)))?;

        // Write manifest
        let manifest_path = version_dir.join("dashflow.toml");
        let manifest_content = toml::to_string_pretty(manifest).map_err(|e| {
            RegistryError::InvalidManifest(format!("Failed to serialize manifest: {}", e))
        })?;
        std::fs::write(&manifest_path, manifest_content)
            .map_err(|e| RegistryError::Io(format!("Failed to write manifest: {}", e)))?;

        // Update index
        let id_str = id.to_string();
        if let Some(entry) = self.index.packages.get_mut(&id_str) {
            if !entry.versions.contains(version) {
                entry.versions.push(version.clone());
                entry.versions.sort();
            }
        } else {
            self.index.packages.insert(
                id_str,
                PackageEntry {
                    id: id.clone(),
                    name: manifest.name.clone(),
                    description: manifest.description.clone(),
                    package_type: manifest.package_type,
                    keywords: manifest.keywords.clone(),
                    versions: vec![version.clone()],
                },
            );
        }

        // Save index
        self.save_index()?;

        Ok(())
    }

    /// Add a package to the registry with trust enforcement (M-199).
    ///
    /// This method validates the package against the provided `TrustConfig` before adding:
    /// - Checks signature requirements (none/any/official/specific keys)
    /// - Verifies minimum trust level
    /// - Rejects vulnerable packages if configured
    /// - Allows unsigned packages from whitelisted namespaces
    ///
    /// # Arguments
    ///
    /// * `manifest` - Package manifest to add
    /// * `trust_config` - Trust configuration to enforce
    /// * `signature` - Optional signature for the package
    /// * `signature_trust_level` - Trust level of the signature (if signature provided)
    /// * `advisory_count` - Number of security advisories for this package
    ///
    /// # Errors
    ///
    /// Returns error if trust requirements are not met.
    pub fn add_with_trust(
        &mut self,
        manifest: &PackageManifest,
        trust_config: &TrustConfig,
        signature: Option<&Signature>,
        signature_trust_level: Option<TrustLevel>,
        advisory_count: usize,
    ) -> RegistryResult<()> {
        let package_name = manifest.id.to_string();
        let namespace = manifest.id.namespace();

        // Check vulnerability requirements
        if trust_config.reject_vulnerable && advisory_count > 0 {
            return Err(RegistryError::VulnerablePackage {
                package: package_name,
                advisory_count,
            });
        }

        // Check if namespace allows unsigned packages
        let allows_unsigned = trust_config.allows_unsigned(namespace);

        // Check signature requirements
        match &trust_config.required_signatures {
            RequiredSignatures::None => {
                // No signature required, proceed
            }
            RequiredSignatures::Any => {
                // Any signature required
                if signature.is_none() && !allows_unsigned {
                    return Err(RegistryError::SignatureRequired {
                        package: package_name,
                    });
                }
            }
            RequiredSignatures::Official => {
                // Official signature required
                if allows_unsigned {
                    // Namespace allows unsigned, proceed
                } else {
                    match signature_trust_level {
                        Some(TrustLevel::Official) => {
                            // Has official signature, proceed
                        }
                        _ => {
                            return Err(RegistryError::OfficialSignatureRequired {
                                package: package_name,
                            });
                        }
                    }
                }
            }
            RequiredSignatures::Keys(required_keys) => {
                // Specific keys required
                if allows_unsigned {
                    // Namespace allows unsigned, proceed
                } else {
                    match signature {
                        Some(sig) => {
                            // Check if signature is from a required key
                            if !required_keys.contains(&sig.key_id) {
                                return Err(RegistryError::SignatureKeyRequired {
                                    package: package_name,
                                    required_key: required_keys.join(", "),
                                });
                            }
                        }
                        None => {
                            return Err(RegistryError::SignatureRequired {
                                package: package_name,
                            });
                        }
                    }
                }
            }
        }

        // Check minimum trust level
        if let Some(minimum_trust) = &trust_config.minimum_trust {
            let actual_trust = signature_trust_level.unwrap_or(TrustLevel::Local);
            if actual_trust < *minimum_trust {
                return Err(RegistryError::InsufficientTrustLevel {
                    package: package_name,
                    required: minimum_trust.as_str().to_string(),
                    actual: actual_trust.as_str().to_string(),
                });
            }
        }

        // All trust checks passed, proceed with add
        self.add(manifest)
    }

    /// Remove a specific version of a package.
    pub fn remove_version(&mut self, id: &PackageId, version: &Version) -> RegistryResult<()> {
        let id_str = id.to_string();

        // Check if version exists
        if !self.version_exists(id, version) {
            return Err(RegistryError::VersionNotFound {
                package: id_str,
                version: version.to_string(),
            });
        }

        // Remove version directory
        let version_dir = self.version_path(id, version);
        if version_dir.exists() {
            std::fs::remove_dir_all(&version_dir).map_err(|e| {
                RegistryError::Io(format!("Failed to remove version directory: {}", e))
            })?;
        }

        // Update index
        if let Some(entry) = self.index.packages.get_mut(&id_str) {
            entry.versions.retain(|v| v != version);

            // Remove package entry if no versions left
            if entry.versions.is_empty() {
                self.index.packages.remove(&id_str);

                // Remove package directory if empty
                let pkg_dir = self.package_path(id);
                if pkg_dir.exists()
                    && std::fs::read_dir(&pkg_dir)
                        .map(|d| d.count() == 0)
                        .unwrap_or(false)
                {
                    let _ = std::fs::remove_dir(&pkg_dir);
                }
            }
        }

        // Save index
        self.save_index()?;

        Ok(())
    }

    /// Remove a package entirely (all versions).
    pub fn remove(&mut self, id: &PackageId) -> RegistryResult<()> {
        let id_str = id.to_string();

        if !self.exists(id) {
            return Err(RegistryError::PackageNotFound(id_str));
        }

        // Remove package directory
        let pkg_dir = self.package_path(id);
        if pkg_dir.exists() {
            std::fs::remove_dir_all(&pkg_dir).map_err(|e| {
                RegistryError::Io(format!("Failed to remove package directory: {}", e))
            })?;
        }

        // Update index
        self.index.packages.remove(&id_str);

        // Save index
        self.save_index()?;

        Ok(())
    }

    /// List all installed packages.
    pub fn list_installed(&self) -> Vec<InstalledPackage> {
        self.index
            .packages
            .values()
            .flat_map(|entry| {
                entry.versions.iter().map(move |version| InstalledPackage {
                    id: entry.id.clone(),
                    name: entry.name.clone(),
                    version: version.clone(),
                    package_type: entry.package_type,
                })
            })
            .collect()
    }

    /// List packages with available updates.
    ///
    /// Returns packages where a newer version exists in the central registry.
    /// Note: This is a stub that returns empty - actual implementation requires
    /// checking against central registry.
    pub fn list_outdated(&self) -> Vec<OutdatedPackage> {
        // Stub implementation - would check against central registry
        Vec::new()
    }

    /// Rebuild the index from disk.
    ///
    /// Scans the package directory and rebuilds the index file.
    pub fn rebuild_index(&mut self) -> RegistryResult<()> {
        let mut new_index = PackageIndex::default();

        // Scan namespace directories
        if let Ok(namespaces) = std::fs::read_dir(&self.root) {
            for ns_entry in namespaces.flatten() {
                if !ns_entry.file_type().is_ok_and(|t| t.is_dir()) {
                    continue;
                }

                let namespace = ns_entry.file_name().to_string_lossy().to_string();
                if namespace == "index.json" {
                    continue;
                }

                // Scan package directories within namespace
                if let Ok(packages) = std::fs::read_dir(ns_entry.path()) {
                    for pkg_entry in packages.flatten() {
                        if !pkg_entry.file_type().is_ok_and(|t| t.is_dir()) {
                            continue;
                        }

                        let name = pkg_entry.file_name().to_string_lossy().to_string();
                        let id = PackageId::new(&namespace, &name);

                        // Scan version directories
                        let mut versions = Vec::new();
                        let mut latest_manifest: Option<PackageManifest> = None;

                        if let Ok(version_dirs) = std::fs::read_dir(pkg_entry.path()) {
                            for ver_entry in version_dirs.flatten() {
                                if !ver_entry.file_type().is_ok_and(|t| t.is_dir()) {
                                    continue;
                                }

                                let version_str =
                                    ver_entry.file_name().to_string_lossy().to_string();
                                if let Some(version) = Version::parse(&version_str) {
                                    // Try to read manifest
                                    let manifest_path = ver_entry.path().join("dashflow.toml");
                                    if manifest_path.exists() {
                                        if let Ok(content) = std::fs::read_to_string(&manifest_path)
                                        {
                                            if let Ok(manifest) =
                                                toml::from_str::<PackageManifest>(&content)
                                            {
                                                if latest_manifest
                                                    .as_ref()
                                                    .map(|m| manifest.version > m.version)
                                                    .unwrap_or(true)
                                                {
                                                    latest_manifest = Some(manifest);
                                                }
                                            }
                                        }
                                    }
                                    versions.push(version);
                                }
                            }
                        }

                        if !versions.is_empty() {
                            versions.sort();

                            let entry = if let Some(manifest) = latest_manifest {
                                PackageEntry {
                                    id: id.clone(),
                                    name: manifest.name,
                                    description: manifest.description,
                                    package_type: manifest.package_type,
                                    keywords: manifest.keywords,
                                    versions,
                                }
                            } else {
                                PackageEntry {
                                    id: id.clone(),
                                    name: name.clone(),
                                    description: String::new(),
                                    package_type: PackageType::NodeLibrary,
                                    keywords: Vec::new(),
                                    versions,
                                }
                            };

                            new_index.packages.insert(id.to_string(), entry);
                        }
                    }
                }
            }
        }

        self.index = new_index;
        self.save_index()?;

        Ok(())
    }

    /// Get a prompt from an installed package.
    ///
    /// The prompt_id can be in two formats:
    /// - `"package-name/prompt-name"` - fully qualified (uses latest version)
    /// - `"namespace/package-name/prompt-name"` - with explicit namespace
    ///
    /// For packages with namespace, use the full package ID format.
    ///
    /// ## Example
    ///
    /// ```rust,ignore
    /// let registry = LocalRegistry::default_path()?;
    ///
    /// // Get a prompt from a package
    /// let prompt = registry.get_prompt("sentiment-pack/analyzer-v2")?;
    ///
    /// // Convert to NodeConfig format
    /// let config = prompt.to_node_config();
    /// ```
    pub fn get_prompt(
        &self,
        prompt_id: &str,
    ) -> RegistryResult<super::prompts::PackagePromptTemplate> {
        use super::prompts::parse_prompt_id;
        use super::types::PackageType;

        // Parse the prompt ID
        let (package_part, prompt_name) =
            parse_prompt_id(prompt_id).map_err(|e| RegistryError::InvalidPath(e.to_string()))?;

        // Find the package
        let package_id = if let Some(pkg) = package_part {
            // Try to find a package with this name (checking all namespaces)
            let matching: Vec<_> = self
                .index
                .packages
                .values()
                .filter(|entry| entry.id.name() == pkg || entry.id.to_string() == pkg)
                .collect();

            if matching.is_empty() {
                return Err(RegistryError::PackageNotFound(pkg));
            }
            if matching.len() > 1 {
                return Err(RegistryError::InvalidPath(format!(
                    "Ambiguous package name '{}'. Use full ID (namespace/name/prompt).",
                    pkg
                )));
            }
            matching[0].id.clone()
        } else {
            return Err(RegistryError::InvalidPath(
                "Prompt ID must include package name (e.g., 'package/prompt')".to_string(),
            ));
        };

        // Get the package entry
        let entry = self
            .get(&package_id)
            .ok_or_else(|| RegistryError::PackageNotFound(package_id.to_string()))?;

        // Verify it's a prompt library
        if entry.package_type != PackageType::PromptLibrary {
            return Err(RegistryError::InvalidManifest(format!(
                "Package '{}' is not a prompt library (type: {})",
                package_id, entry.package_type
            )));
        }

        // Get the latest version
        let version = entry
            .latest()
            .ok_or_else(|| RegistryError::VersionNotFound {
                package: package_id.to_string(),
                version: "latest".to_string(),
            })?;

        // Load the prompt library
        let library = self.read_prompt_library(&package_id, version)?;

        // Find the specific prompt
        library.get_prompt(&prompt_name).cloned().ok_or_else(|| {
            RegistryError::InvalidPath(format!(
                "Prompt '{}' not found in package '{}'",
                prompt_name, package_id
            ))
        })
    }

    /// Get a prompt from a specific package version.
    pub fn get_prompt_versioned(
        &self,
        package_id: &PackageId,
        version: &Version,
        prompt_name: &str,
    ) -> RegistryResult<super::prompts::PackagePromptTemplate> {
        let library = self.read_prompt_library(package_id, version)?;
        library.get_prompt(prompt_name).cloned().ok_or_else(|| {
            RegistryError::InvalidPath(format!(
                "Prompt '{}' not found in package '{}@{}'",
                prompt_name, package_id, version
            ))
        })
    }

    /// Read a prompt library from a package.
    ///
    /// Looks for `prompts.toml` or `prompts.json` in the package directory.
    pub fn read_prompt_library(
        &self,
        id: &PackageId,
        version: &Version,
    ) -> RegistryResult<super::prompts::PromptLibrary> {
        let version_dir = self.version_path(id, version);

        if !version_dir.exists() {
            return Err(RegistryError::VersionNotFound {
                package: id.to_string(),
                version: version.to_string(),
            });
        }

        // Try prompts.toml first, then prompts.json
        let toml_path = version_dir.join("prompts.toml");
        let json_path = version_dir.join("prompts.json");

        if toml_path.exists() {
            let content = std::fs::read_to_string(&toml_path)
                .map_err(|e| RegistryError::Io(format!("Failed to read prompts.toml: {}", e)))?;
            toml::from_str(&content).map_err(|e| {
                RegistryError::InvalidManifest(format!("Failed to parse prompts.toml: {}", e))
            })
        } else if json_path.exists() {
            let content = std::fs::read_to_string(&json_path)
                .map_err(|e| RegistryError::Io(format!("Failed to read prompts.json: {}", e)))?;
            serde_json::from_str(&content).map_err(|e| {
                RegistryError::InvalidManifest(format!("Failed to parse prompts.json: {}", e))
            })
        } else {
            // Return empty library if no prompts file exists
            Ok(super::prompts::PromptLibrary::new())
        }
    }

    /// List all prompts in an installed package.
    pub fn list_prompts(&self, id: &PackageId) -> RegistryResult<Vec<String>> {
        let entry = self
            .get(id)
            .ok_or_else(|| RegistryError::PackageNotFound(id.to_string()))?;

        if entry.package_type != PackageType::PromptLibrary {
            return Err(RegistryError::InvalidManifest(format!(
                "Package '{}' is not a prompt library",
                id
            )));
        }

        let version = entry
            .latest()
            .ok_or_else(|| RegistryError::VersionNotFound {
                package: id.to_string(),
                version: "latest".to_string(),
            })?;

        let library = self.read_prompt_library(id, version)?;
        Ok(library
            .prompt_names()
            .into_iter()
            .map(String::from)
            .collect())
    }
}

/// Expand tilde in path to home directory.
fn expand_tilde(path: &Path) -> PathBuf {
    if let Ok(stripped) = path.strip_prefix("~") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    }
    path.to_path_buf()
}

/// Package index for fast lookups.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PackageIndex {
    /// Map of package ID string to entry
    pub packages: HashMap<String, PackageEntry>,
    /// Index version (for migrations)
    #[serde(default = "default_index_version")]
    pub version: u32,
}

fn default_index_version() -> u32 {
    1
}

impl PackageIndex {
    /// Get the number of packages in the index.
    pub fn len(&self) -> usize {
        self.packages.len()
    }

    /// Check if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }

    /// Get total number of package versions.
    pub fn total_versions(&self) -> usize {
        self.packages.values().map(|e| e.versions.len()).sum()
    }
}

/// Unified Registry trait implementation for PackageIndex
impl Registry<PackageEntry> for PackageIndex {
    fn get(&self, key: &str) -> Option<&PackageEntry> {
        self.packages.get(key)
    }

    fn contains(&self, key: &str) -> bool {
        self.packages.contains_key(key)
    }

    fn len(&self) -> usize {
        self.packages.len()
    }
}

/// Package entry in the index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageEntry {
    /// Package ID
    pub id: PackageId,
    /// Human-readable name
    pub name: String,
    /// Short description
    pub description: String,
    /// Package type
    pub package_type: PackageType,
    /// Keywords for search
    pub keywords: Vec<String>,
    /// Available versions (sorted)
    pub versions: Vec<Version>,
}

impl PackageEntry {
    /// Get the latest version.
    pub fn latest(&self) -> Option<&Version> {
        self.versions.iter().max()
    }
}

/// An installed package.
#[derive(Debug, Clone)]
pub struct InstalledPackage {
    /// Package ID
    pub id: PackageId,
    /// Human-readable name
    pub name: String,
    /// Installed version
    pub version: Version,
    /// Package type
    pub package_type: PackageType,
}

/// A package with an available update.
#[derive(Debug, Clone)]
pub struct OutdatedPackage {
    /// Package ID
    pub id: PackageId,
    /// Currently installed version
    pub installed: Version,
    /// Latest available version
    pub latest: Version,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packages::manifest::{Author, PackageManifest};
    use tempfile::tempdir;

    fn create_test_manifest(
        namespace: &str,
        name: &str,
        version: (u32, u32, u32),
    ) -> PackageManifest {
        PackageManifest::builder()
            .id(namespace, name)
            .name(format!("{} {}", namespace, name))
            .version(version.0, version.1, version.2)
            .package_type(PackageType::NodeLibrary)
            .description(format!("Test package {}/{}", namespace, name))
            .author(Author::new("Test Author"))
            .keywords(["test", "example"])
            .build()
    }

    #[test]
    fn test_local_registry_new() {
        let temp_dir = tempdir().unwrap();
        let registry = LocalRegistry::new(temp_dir.path()).unwrap();

        assert!(registry.root().exists());
        assert!(registry.index().is_empty());
    }

    #[test]
    fn test_registry_add_and_get() {
        let temp_dir = tempdir().unwrap();
        let mut registry = LocalRegistry::new(temp_dir.path()).unwrap();

        let manifest = create_test_manifest("test", "my-package", (1, 0, 0));
        registry.add(&manifest).unwrap();

        let id = PackageId::new("test", "my-package");
        assert!(registry.exists(&id));

        let entry = registry.get(&id).unwrap();
        assert_eq!(entry.name, "test my-package");
        assert_eq!(entry.versions.len(), 1);
    }

    #[test]
    fn test_registry_add_multiple_versions() {
        let temp_dir = tempdir().unwrap();
        let mut registry = LocalRegistry::new(temp_dir.path()).unwrap();

        let id = PackageId::new("test", "versioned");

        let v1 = create_test_manifest("test", "versioned", (1, 0, 0));
        let v2 = create_test_manifest("test", "versioned", (1, 1, 0));
        let v3 = create_test_manifest("test", "versioned", (2, 0, 0));

        registry.add(&v1).unwrap();
        registry.add(&v2).unwrap();
        registry.add(&v3).unwrap();

        let entry = registry.get(&id).unwrap();
        assert_eq!(entry.versions.len(), 3);

        let latest = registry.latest_version(&id).unwrap();
        assert_eq!(*latest, Version::new(2, 0, 0));
    }

    #[test]
    fn test_registry_search() {
        let temp_dir = tempdir().unwrap();
        let mut registry = LocalRegistry::new(temp_dir.path()).unwrap();

        let m1 = PackageManifest::builder()
            .id("dashflow", "sentiment-analysis")
            .name("Sentiment Analysis")
            .version(1, 0, 0)
            .package_type(PackageType::NodeLibrary)
            .description("Analyze sentiment in text")
            .author(Author::new("Test"))
            .keywords(["nlp", "sentiment", "text"])
            .build();

        let m2 = PackageManifest::builder()
            .id("community", "code-review")
            .name("Code Review")
            .version(1, 0, 0)
            .package_type(PackageType::GraphTemplate)
            .description("Automated code review")
            .author(Author::new("Test"))
            .keywords(["code", "review", "automation"])
            .build();

        registry.add(&m1).unwrap();
        registry.add(&m2).unwrap();

        // Search by name
        let results = registry.search("sentiment");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id.name(), "sentiment-analysis");

        // Search by keyword
        let results = registry.search("nlp");
        assert_eq!(results.len(), 1);

        // Search by description
        let results = registry.search("automated");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id.name(), "code-review");
    }

    #[test]
    fn test_registry_list_by_type() {
        let temp_dir = tempdir().unwrap();
        let mut registry = LocalRegistry::new(temp_dir.path()).unwrap();

        let m1 = create_test_manifest("test", "lib1", (1, 0, 0));
        let m2 = PackageManifest::builder()
            .id("test", "template1")
            .name("Template")
            .version(1, 0, 0)
            .package_type(PackageType::GraphTemplate)
            .description("Test template")
            .author(Author::new("Test"))
            .build();

        registry.add(&m1).unwrap();
        registry.add(&m2).unwrap();

        let libs = registry.list_by_type(PackageType::NodeLibrary);
        assert_eq!(libs.len(), 1);

        let templates = registry.list_by_type(PackageType::GraphTemplate);
        assert_eq!(templates.len(), 1);
    }

    #[test]
    fn test_registry_version_matching() {
        let temp_dir = tempdir().unwrap();
        let mut registry = LocalRegistry::new(temp_dir.path()).unwrap();

        let id = PackageId::new("test", "versions");

        for (major, minor, patch) in [(1, 0, 0), (1, 1, 0), (1, 2, 0), (2, 0, 0)] {
            let m = create_test_manifest("test", "versions", (major, minor, patch));
            registry.add(&m).unwrap();
        }

        // Test caret matching
        let req = VersionReq::caret(Version::new(1, 0, 0));
        let matching = registry.find_matching_versions(&id, &req);
        assert_eq!(matching.len(), 3); // 1.0.0, 1.1.0, 1.2.0

        // Test best matching
        let best = registry.best_matching_version(&id, &req).unwrap();
        assert_eq!(*best, Version::new(1, 2, 0));
    }

    #[test]
    fn test_registry_remove_version() {
        let temp_dir = tempdir().unwrap();
        let mut registry = LocalRegistry::new(temp_dir.path()).unwrap();

        let id = PackageId::new("test", "remove-test");
        let m1 = create_test_manifest("test", "remove-test", (1, 0, 0));
        let m2 = create_test_manifest("test", "remove-test", (1, 1, 0));

        registry.add(&m1).unwrap();
        registry.add(&m2).unwrap();

        assert_eq!(registry.get(&id).unwrap().versions.len(), 2);

        registry
            .remove_version(&id, &Version::new(1, 0, 0))
            .unwrap();

        assert_eq!(registry.get(&id).unwrap().versions.len(), 1);
        assert!(!registry.version_exists(&id, &Version::new(1, 0, 0)));
        assert!(registry.version_exists(&id, &Version::new(1, 1, 0)));
    }

    #[test]
    fn test_registry_remove_package() {
        let temp_dir = tempdir().unwrap();
        let mut registry = LocalRegistry::new(temp_dir.path()).unwrap();

        let id = PackageId::new("test", "to-remove");
        let m1 = create_test_manifest("test", "to-remove", (1, 0, 0));

        registry.add(&m1).unwrap();
        assert!(registry.exists(&id));

        registry.remove(&id).unwrap();
        assert!(!registry.exists(&id));
    }

    #[test]
    fn test_registry_list_installed() {
        let temp_dir = tempdir().unwrap();
        let mut registry = LocalRegistry::new(temp_dir.path()).unwrap();

        let m1 = create_test_manifest("ns1", "pkg1", (1, 0, 0));
        let m2 = create_test_manifest("ns1", "pkg1", (1, 1, 0));
        let m3 = create_test_manifest("ns2", "pkg2", (2, 0, 0));

        registry.add(&m1).unwrap();
        registry.add(&m2).unwrap();
        registry.add(&m3).unwrap();

        let installed = registry.list_installed();
        assert_eq!(installed.len(), 3);
    }

    #[test]
    fn test_registry_read_manifest() {
        let temp_dir = tempdir().unwrap();
        let mut registry = LocalRegistry::new(temp_dir.path()).unwrap();

        let manifest = create_test_manifest("test", "readable", (1, 0, 0));
        registry.add(&manifest).unwrap();

        let id = PackageId::new("test", "readable");
        let read_manifest = registry.read_manifest(&id, &Version::new(1, 0, 0)).unwrap();

        assert_eq!(read_manifest.id, manifest.id);
        assert_eq!(read_manifest.version, manifest.version);
        assert_eq!(read_manifest.description, manifest.description);
    }

    #[test]
    fn test_registry_rebuild_index() {
        let temp_dir = tempdir().unwrap();
        let mut registry = LocalRegistry::new(temp_dir.path()).unwrap();

        // Add some packages
        let m1 = create_test_manifest("test", "rebuild1", (1, 0, 0));
        let m2 = create_test_manifest("test", "rebuild2", (2, 0, 0));
        registry.add(&m1).unwrap();
        registry.add(&m2).unwrap();

        // Clear index in memory
        registry.index = PackageIndex::default();
        assert!(registry.index.is_empty());

        // Rebuild
        registry.rebuild_index().unwrap();

        assert_eq!(registry.index.len(), 2);
        assert!(registry.exists(&PackageId::new("test", "rebuild1")));
        assert!(registry.exists(&PackageId::new("test", "rebuild2")));
    }

    #[test]
    fn test_package_entry_latest() {
        let entry = PackageEntry {
            id: PackageId::new("test", "pkg"),
            name: "Test".to_string(),
            description: "Test package".to_string(),
            package_type: PackageType::NodeLibrary,
            keywords: vec![],
            versions: vec![
                Version::new(1, 0, 0),
                Version::new(2, 0, 0),
                Version::new(1, 5, 0),
            ],
        };

        assert_eq!(entry.latest(), Some(&Version::new(2, 0, 0)));
    }

    #[test]
    fn test_expand_tilde() {
        // Test that tilde is expanded
        let path = expand_tilde(Path::new("~/.dashflow/test"));

        // Should not start with ~
        assert!(!path.starts_with("~"));

        // Should contain dashflow/test
        assert!(path.to_string_lossy().contains(".dashflow"));
    }

    #[test]
    fn test_registry_error_display() {
        let err = RegistryError::PackageNotFound("test/pkg".to_string());
        assert!(err.to_string().contains("test/pkg"));

        let err = RegistryError::VersionNotFound {
            package: "test/pkg".to_string(),
            version: "1.0.0".to_string(),
        };
        assert!(err.to_string().contains("1.0.0"));
    }

    #[test]
    fn test_registry_get_prompt() {
        use crate::packages::prompts::{PackagePromptTemplate, PromptLibrary};

        let temp_dir = tempdir().unwrap();
        let mut registry = LocalRegistry::new(temp_dir.path()).unwrap();

        // Create a prompt library package
        let manifest = PackageManifest::builder()
            .id("test", "sentiment-prompts")
            .name("Sentiment Prompts")
            .version(1, 0, 0)
            .package_type(PackageType::PromptLibrary)
            .description("Prompts for sentiment analysis")
            .author(Author::new("Test Author"))
            .build();

        registry.add(&manifest).unwrap();

        // Create a prompts.json file
        let prompts_lib = PromptLibrary::new()
            .with_prompt(
                PackagePromptTemplate::new("analyzer-v1", "You are a sentiment analyzer.")
                    .with_description("Basic sentiment analyzer")
                    .with_temperature(0.7),
            )
            .with_prompt(
                PackagePromptTemplate::new(
                    "analyzer-v2",
                    "You are an advanced sentiment analyzer. Be thorough.",
                )
                .with_description("Advanced sentiment analyzer")
                .with_temperature(0.5)
                .with_max_tokens(1000),
            );

        let version_dir = registry.version_path(&manifest.id, &manifest.version);
        let prompts_path = version_dir.join("prompts.json");
        let prompts_content = serde_json::to_string_pretty(&prompts_lib).unwrap();
        std::fs::write(&prompts_path, prompts_content).unwrap();

        // Get a prompt
        let prompt = registry
            .get_prompt("sentiment-prompts/analyzer-v1")
            .unwrap();
        assert_eq!(prompt.name, "analyzer-v1");
        assert_eq!(prompt.system, "You are a sentiment analyzer.");
        assert_eq!(prompt.recommended_temperature, Some(0.7));

        // Get another prompt
        let prompt = registry
            .get_prompt("sentiment-prompts/analyzer-v2")
            .unwrap();
        assert_eq!(prompt.name, "analyzer-v2");
        assert_eq!(prompt.recommended_max_tokens, Some(1000));

        // Convert to node config
        let config = prompt.to_node_config();
        assert_eq!(
            config["system_prompt"],
            "You are an advanced sentiment analyzer. Be thorough."
        );
        assert_eq!(config["temperature"], 0.5);
        assert_eq!(config["max_tokens"], 1000);
    }

    #[test]
    fn test_registry_get_prompt_not_prompt_library() {
        let temp_dir = tempdir().unwrap();
        let mut registry = LocalRegistry::new(temp_dir.path()).unwrap();

        // Create a node library (not a prompt library)
        let manifest = create_test_manifest("test", "nodes", (1, 0, 0));
        registry.add(&manifest).unwrap();

        // Should fail because it's not a prompt library
        let result = registry.get_prompt("nodes/some-prompt");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("not a prompt library"));
    }

    #[test]
    fn test_registry_get_prompt_not_found() {
        use crate::packages::prompts::PromptLibrary;

        let temp_dir = tempdir().unwrap();
        let mut registry = LocalRegistry::new(temp_dir.path()).unwrap();

        // Create a prompt library package with no prompts
        let manifest = PackageManifest::builder()
            .id("test", "empty-prompts")
            .name("Empty Prompts")
            .version(1, 0, 0)
            .package_type(PackageType::PromptLibrary)
            .description("Empty prompt library")
            .author(Author::new("Test Author"))
            .build();

        registry.add(&manifest).unwrap();

        // Create an empty prompts.json file
        let version_dir = registry.version_path(&manifest.id, &manifest.version);
        let prompts_path = version_dir.join("prompts.json");
        let prompts_content = serde_json::to_string(&PromptLibrary::new()).unwrap();
        std::fs::write(&prompts_path, prompts_content).unwrap();

        // Should fail because prompt doesn't exist
        let result = registry.get_prompt("empty-prompts/nonexistent");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_registry_list_prompts() {
        use crate::packages::prompts::{PackagePromptTemplate, PromptLibrary};

        let temp_dir = tempdir().unwrap();
        let mut registry = LocalRegistry::new(temp_dir.path()).unwrap();

        // Create a prompt library package
        let manifest = PackageManifest::builder()
            .id("test", "list-prompts")
            .name("List Prompts")
            .version(1, 0, 0)
            .package_type(PackageType::PromptLibrary)
            .description("Test prompts")
            .author(Author::new("Test Author"))
            .build();

        registry.add(&manifest).unwrap();

        // Create prompts
        let prompts_lib = PromptLibrary::new()
            .with_prompt(PackagePromptTemplate::new("prompt-a", "System A"))
            .with_prompt(PackagePromptTemplate::new("prompt-b", "System B"))
            .with_prompt(PackagePromptTemplate::new("prompt-c", "System C"));

        let version_dir = registry.version_path(&manifest.id, &manifest.version);
        let prompts_path = version_dir.join("prompts.json");
        std::fs::write(&prompts_path, serde_json::to_string(&prompts_lib).unwrap()).unwrap();

        // List prompts
        let prompts = registry.list_prompts(&manifest.id).unwrap();
        assert_eq!(prompts.len(), 3);
        assert!(prompts.contains(&"prompt-a".to_string()));
        assert!(prompts.contains(&"prompt-b".to_string()));
        assert!(prompts.contains(&"prompt-c".to_string()));
    }

    #[test]
    fn test_registry_read_prompt_library_toml() {
        #[allow(unused_imports)]
        use crate::packages::prompts::{PackagePromptTemplate, PromptLibrary};

        let temp_dir = tempdir().unwrap();
        let mut registry = LocalRegistry::new(temp_dir.path()).unwrap();

        // Create a prompt library package
        let manifest = PackageManifest::builder()
            .id("test", "toml-prompts")
            .name("TOML Prompts")
            .version(1, 0, 0)
            .package_type(PackageType::PromptLibrary)
            .description("TOML format prompts")
            .author(Author::new("Test Author"))
            .build();

        registry.add(&manifest).unwrap();

        // Create prompts.toml file
        let version_dir = registry.version_path(&manifest.id, &manifest.version);
        let prompts_path = version_dir.join("prompts.toml");
        let toml_content = r#"
description = "Test prompts in TOML"

[[prompts]]
name = "toml-prompt"
system = "You are a TOML-based assistant."
recommended_temperature = 0.8
"#;
        std::fs::write(&prompts_path, toml_content).unwrap();

        // Read the library
        let library = registry
            .read_prompt_library(&manifest.id, &manifest.version)
            .unwrap();
        assert_eq!(library.len(), 1);

        let prompt = library.get_prompt("toml-prompt").unwrap();
        assert_eq!(prompt.system, "You are a TOML-based assistant.");
        assert_eq!(prompt.recommended_temperature, Some(0.8));
    }

    // ==================== M-199: Trust Enforcement Tests ====================

    #[test]
    fn test_add_with_trust_no_signature_required() {
        let temp_dir = tempdir().unwrap();
        let mut registry = LocalRegistry::new(temp_dir.path()).unwrap();

        let manifest = create_test_manifest("test", "no-sig", (1, 0, 0));
        let trust_config = TrustConfig::default(); // RequiredSignatures::None by default

        // Should succeed without signature
        registry
            .add_with_trust(&manifest, &trust_config, None, None, 0)
            .unwrap();

        let id = PackageId::new("test", "no-sig");
        assert!(registry.exists(&id));
    }

    #[test]
    fn test_add_with_trust_any_signature_required() {
        let temp_dir = tempdir().unwrap();
        let mut registry = LocalRegistry::new(temp_dir.path()).unwrap();

        let manifest = create_test_manifest("test", "any-sig", (1, 0, 0));
        let trust_config = TrustConfig {
            required_signatures: RequiredSignatures::Any,
            ..Default::default()
        };

        // Should fail without signature
        let result = registry.add_with_trust(&manifest, &trust_config, None, None, 0);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, RegistryError::SignatureRequired { .. }));
    }

    #[test]
    fn test_add_with_trust_allows_unsigned_namespace() {
        let temp_dir = tempdir().unwrap();
        let mut registry = LocalRegistry::new(temp_dir.path()).unwrap();

        let manifest = create_test_manifest("local", "unsigned-ok", (1, 0, 0));
        let trust_config = TrustConfig {
            required_signatures: RequiredSignatures::Any,
            allow_unsigned: vec!["local/*".to_string()],
            ..Default::default()
        };

        // Should succeed even without signature because namespace is whitelisted
        registry
            .add_with_trust(&manifest, &trust_config, None, None, 0)
            .unwrap();

        let id = PackageId::new("local", "unsigned-ok");
        assert!(registry.exists(&id));
    }

    #[test]
    fn test_add_with_trust_official_signature_required() {
        let temp_dir = tempdir().unwrap();
        let mut registry = LocalRegistry::new(temp_dir.path()).unwrap();

        let manifest = create_test_manifest("dashflow", "official-pkg", (1, 0, 0));
        let trust_config = TrustConfig {
            required_signatures: RequiredSignatures::Official,
            ..Default::default()
        };

        // Should fail without official signature
        let result = registry.add_with_trust(&manifest, &trust_config, None, None, 0);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            RegistryError::OfficialSignatureRequired { .. }
        ));

        // Should succeed with official signature
        let result = registry.add_with_trust(
            &manifest,
            &trust_config,
            None,
            Some(TrustLevel::Official),
            0,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_add_with_trust_rejects_vulnerable() {
        let temp_dir = tempdir().unwrap();
        let mut registry = LocalRegistry::new(temp_dir.path()).unwrap();

        let manifest = create_test_manifest("test", "vulnerable", (1, 0, 0));
        let trust_config = TrustConfig {
            reject_vulnerable: true,
            ..Default::default()
        };

        // Should fail with vulnerabilities
        let result = registry.add_with_trust(&manifest, &trust_config, None, None, 3);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            RegistryError::VulnerablePackage {
                advisory_count: 3,
                ..
            }
        ));
    }

    #[test]
    fn test_add_with_trust_allows_vulnerable_when_disabled() {
        let temp_dir = tempdir().unwrap();
        let mut registry = LocalRegistry::new(temp_dir.path()).unwrap();

        let manifest = create_test_manifest("test", "vulnerable-ok", (1, 0, 0));
        let trust_config = TrustConfig {
            reject_vulnerable: false,
            ..Default::default()
        };

        // Should succeed even with vulnerabilities when reject_vulnerable is false
        registry
            .add_with_trust(&manifest, &trust_config, None, None, 5)
            .unwrap();

        let id = PackageId::new("test", "vulnerable-ok");
        assert!(registry.exists(&id));
    }

    #[test]
    fn test_add_with_trust_minimum_trust_level() {
        let temp_dir = tempdir().unwrap();
        let mut registry = LocalRegistry::new(temp_dir.path()).unwrap();

        let manifest = create_test_manifest("test", "trust-level", (1, 0, 0));
        let trust_config = TrustConfig {
            minimum_trust: Some(TrustLevel::Verified),
            ..Default::default()
        };

        // Should fail with insufficient trust
        let result = registry.add_with_trust(
            &manifest,
            &trust_config,
            None,
            Some(TrustLevel::Community),
            0,
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, RegistryError::InsufficientTrustLevel { .. }));

        // Create a new manifest to avoid "package exists" error
        let manifest2 = create_test_manifest("test", "trust-level-ok", (1, 0, 0));

        // Should succeed with sufficient trust
        let result = registry.add_with_trust(
            &manifest2,
            &trust_config,
            None,
            Some(TrustLevel::Verified),
            0,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_add_with_trust_specific_key_required() {
        let temp_dir = tempdir().unwrap();
        let mut registry = LocalRegistry::new(temp_dir.path()).unwrap();

        let manifest = create_test_manifest("test", "key-pkg", (1, 0, 0));
        let trust_config = TrustConfig {
            required_signatures: RequiredSignatures::Keys(vec![
                "company-key".to_string(),
                "backup-key".to_string(),
            ]),
            ..Default::default()
        };

        // Should fail without signature
        let result = registry.add_with_trust(&manifest, &trust_config, None, None, 0);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RegistryError::SignatureRequired { .. }
        ));

        // Should fail with wrong key
        let wrong_sig = Signature::new(
            "wrong-key",
            crate::packages::SignatureAlgorithm::Ed25519,
            "dummy-signature",
            crate::packages::SignedContent::ManifestHash {
                hash: "abc123".to_string(),
                algorithm: crate::packages::HashAlgorithm::Sha256,
            },
            "2025-01-01T00:00:00Z",
        );
        let manifest2 = create_test_manifest("test", "key-pkg-wrong", (1, 0, 0));
        let result = registry.add_with_trust(
            &manifest2,
            &trust_config,
            Some(&wrong_sig),
            Some(TrustLevel::Verified),
            0,
        );
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RegistryError::SignatureKeyRequired { .. }
        ));

        // Should succeed with correct key
        let correct_sig = Signature::new(
            "company-key",
            crate::packages::SignatureAlgorithm::Ed25519,
            "dummy-signature",
            crate::packages::SignedContent::ManifestHash {
                hash: "abc123".to_string(),
                algorithm: crate::packages::HashAlgorithm::Sha256,
            },
            "2025-01-01T00:00:00Z",
        );
        let manifest3 = create_test_manifest("test", "key-pkg-ok", (1, 0, 0));
        let result = registry.add_with_trust(
            &manifest3,
            &trust_config,
            Some(&correct_sig),
            Some(TrustLevel::Verified),
            0,
        );
        assert!(result.is_ok());
    }
}
