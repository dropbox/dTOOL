// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for package manifest
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]

//! Package manifest for DashFlow packages.
//!
//! The manifest defines metadata, dependencies, trust, and capabilities for a package.
//! Manifests are stored in `dashflow.toml` at the package root.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::types::{HashAlgorithm, PackageId, PackageType, Signature, Version, VersionReq};

/// Package manifest (dashflow.toml).
///
/// This is the complete metadata for a package, including identity, authorship,
/// dependencies, trust information, and capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManifest {
    // === Identity ===
    /// Unique package identifier (e.g., "dashflow/sentiment-analysis")
    pub id: PackageId,
    /// Human-readable name
    pub name: String,
    /// Semantic version
    pub version: Version,
    /// Package type
    pub package_type: PackageType,

    // === Metadata ===
    /// Short description (one line)
    pub description: String,
    /// Long description (markdown)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readme: Option<String>,
    /// Keywords for search
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    /// Category tags
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<String>,
    /// License (SPDX identifier)
    #[serde(default = "default_license")]
    pub license: String,
    /// Repository URL
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    /// Documentation URL
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
    /// Homepage URL
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,

    // === Authorship ===
    /// Original author
    pub author: Author,
    /// Contributors
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contributors: Vec<Contributor>,
    /// Maintainers (can publish updates)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub maintainers: Vec<Maintainer>,

    // === Trust ===
    /// Cryptographic signatures
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signatures: Vec<Signature>,
    /// Lineage (derived from other packages)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lineage: Option<Lineage>,
    /// Security audit status
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit: Option<AuditStatus>,

    // === Dependencies ===
    /// Required packages
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<Dependency>,
    /// Optional packages (feature-gated)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional_dependencies: Vec<OptionalDependency>,
    /// DashFlow version requirement
    #[serde(default = "default_dashflow_version")]
    pub dashflow_version: VersionReq,

    // === Capabilities ===
    /// Required permissions
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<Permission>,
    /// Provided capabilities
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provides: Vec<Capability>,

    // === Extra metadata ===
    /// Custom metadata fields
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

fn default_license() -> String {
    "MIT".to_string()
}

fn default_dashflow_version() -> VersionReq {
    VersionReq::at_least(Version::new(1, 11, 0))
}

impl PackageManifest {
    /// Create a new package manifest builder.
    pub fn builder() -> PackageManifestBuilder {
        PackageManifestBuilder::default()
    }

    /// Get the content hash of this manifest (for signing).
    ///
    /// # Errors
    /// Returns an error if the manifest cannot be serialized to JSON.
    pub fn content_hash(&self, algorithm: HashAlgorithm) -> Result<String, serde_json::Error> {
        use sha2::{Digest, Sha256, Sha384, Sha512};

        let json = serde_json::to_string(self)?;
        let hash = match algorithm {
            HashAlgorithm::Sha256 => {
                let hash = Sha256::digest(json.as_bytes());
                format!("sha256:{:x}", hash)
            }
            HashAlgorithm::Sha384 => {
                let hash = Sha384::digest(json.as_bytes());
                format!("sha384:{:x}", hash)
            }
            HashAlgorithm::Sha512 => {
                let hash = Sha512::digest(json.as_bytes());
                format!("sha512:{:x}", hash)
            }
            HashAlgorithm::Blake3 => {
                let hash = blake3::hash(json.as_bytes());
                format!("blake3:{}", hash.to_hex())
            }
        };
        Ok(hash)
    }

    /// Check if this package is signed.
    pub fn is_signed(&self) -> bool {
        !self.signatures.is_empty()
    }

    /// Check if this package has been audited.
    pub fn is_audited(&self) -> bool {
        self.audit
            .as_ref()
            .map(|a| a.status == AuditStatusLevel::Verified)
            .unwrap_or(false)
    }

    /// Check if this package is derived from another.
    pub fn is_derived(&self) -> bool {
        self.lineage
            .as_ref()
            .map(|l| l.derived_from.is_some())
            .unwrap_or(false)
    }

    /// Get all dependency package IDs.
    pub fn dependency_ids(&self) -> Vec<&PackageId> {
        self.dependencies.iter().map(|d| &d.package).collect()
    }
}

/// Builder for PackageManifest.
#[derive(Debug, Default)]
pub struct PackageManifestBuilder {
    id: Option<PackageId>,
    name: Option<String>,
    version: Option<Version>,
    package_type: Option<PackageType>,
    description: Option<String>,
    readme: Option<String>,
    keywords: Vec<String>,
    categories: Vec<String>,
    license: Option<String>,
    repository: Option<String>,
    documentation: Option<String>,
    homepage: Option<String>,
    author: Option<Author>,
    contributors: Vec<Contributor>,
    maintainers: Vec<Maintainer>,
    signatures: Vec<Signature>,
    lineage: Option<Lineage>,
    audit: Option<AuditStatus>,
    dependencies: Vec<Dependency>,
    optional_dependencies: Vec<OptionalDependency>,
    dashflow_version: Option<VersionReq>,
    permissions: Vec<Permission>,
    provides: Vec<Capability>,
    metadata: HashMap<String, serde_json::Value>,
}

impl PackageManifestBuilder {
    /// Set the package ID.
    pub fn id(mut self, namespace: impl Into<String>, name: impl Into<String>) -> Self {
        self.id = Some(PackageId::new(namespace, name));
        self
    }

    /// Set the human-readable name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the version.
    pub fn version(mut self, major: u32, minor: u32, patch: u32) -> Self {
        self.version = Some(Version::new(major, minor, patch));
        self
    }

    /// Set the version from a Version struct.
    pub fn version_from(mut self, version: Version) -> Self {
        self.version = Some(version);
        self
    }

    /// Set the package type.
    pub fn package_type(mut self, package_type: PackageType) -> Self {
        self.package_type = Some(package_type);
        self
    }

    /// Set the description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the readme (markdown).
    pub fn readme(mut self, readme: impl Into<String>) -> Self {
        self.readme = Some(readme.into());
        self
    }

    /// Add keywords.
    pub fn keywords(mut self, keywords: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.keywords.extend(keywords.into_iter().map(Into::into));
        self
    }

    /// Add categories.
    pub fn categories(mut self, categories: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.categories
            .extend(categories.into_iter().map(Into::into));
        self
    }

    /// Set the license.
    pub fn license(mut self, license: impl Into<String>) -> Self {
        self.license = Some(license.into());
        self
    }

    /// Set the repository URL.
    pub fn repository(mut self, url: impl Into<String>) -> Self {
        self.repository = Some(url.into());
        self
    }

    /// Set the documentation URL.
    pub fn documentation(mut self, url: impl Into<String>) -> Self {
        self.documentation = Some(url.into());
        self
    }

    /// Set the homepage URL.
    pub fn homepage(mut self, url: impl Into<String>) -> Self {
        self.homepage = Some(url.into());
        self
    }

    /// Set the author.
    pub fn author(mut self, author: Author) -> Self {
        self.author = Some(author);
        self
    }

    /// Add a contributor.
    pub fn contributor(mut self, contributor: Contributor) -> Self {
        self.contributors.push(contributor);
        self
    }

    /// Add a maintainer.
    pub fn maintainer(mut self, maintainer: Maintainer) -> Self {
        self.maintainers.push(maintainer);
        self
    }

    /// Add a dependency.
    pub fn dependency(mut self, package: PackageId, version: VersionReq) -> Self {
        self.dependencies.push(Dependency { package, version });
        self
    }

    /// Set DashFlow version requirement.
    pub fn dashflow_version(mut self, req: VersionReq) -> Self {
        self.dashflow_version = Some(req);
        self
    }

    /// Add a required permission.
    pub fn permission(mut self, permission: Permission) -> Self {
        self.permissions.push(permission);
        self
    }

    /// Add a provided capability.
    pub fn provides(mut self, capability: Capability) -> Self {
        self.provides.push(capability);
        self
    }

    /// Set lineage information.
    pub fn lineage(mut self, lineage: Lineage) -> Self {
        self.lineage = Some(lineage);
        self
    }

    /// Set audit status.
    pub fn audit(mut self, audit: AuditStatus) -> Self {
        self.audit = Some(audit);
        self
    }

    /// Add custom metadata.
    pub fn metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Build the manifest.
    ///
    /// # Panics
    ///
    /// Panics if required fields (id, name, version, package_type, description, author) are not set.
    pub fn build(self) -> PackageManifest {
        PackageManifest {
            id: self.id.expect("Package ID is required"),
            name: self.name.expect("Package name is required"),
            version: self.version.expect("Version is required"),
            package_type: self.package_type.expect("Package type is required"),
            description: self.description.expect("Description is required"),
            readme: self.readme,
            keywords: self.keywords,
            categories: self.categories,
            license: self.license.unwrap_or_else(default_license),
            repository: self.repository,
            documentation: self.documentation,
            homepage: self.homepage,
            author: self.author.expect("Author is required"),
            contributors: self.contributors,
            maintainers: self.maintainers,
            signatures: self.signatures,
            lineage: self.lineage,
            audit: self.audit,
            dependencies: self.dependencies,
            optional_dependencies: self.optional_dependencies,
            dashflow_version: self
                .dashflow_version
                .unwrap_or_else(default_dashflow_version),
            permissions: self.permissions,
            provides: self.provides,
            metadata: self.metadata,
        }
    }

    /// Try to build the manifest, returning an error if required fields are missing.
    pub fn try_build(self) -> Result<PackageManifest, String> {
        let id = self.id.ok_or("Package ID is required")?;
        let name = self.name.ok_or("Package name is required")?;
        let version = self.version.ok_or("Version is required")?;
        let package_type = self.package_type.ok_or("Package type is required")?;
        let description = self.description.ok_or("Description is required")?;
        let author = self.author.ok_or("Author is required")?;

        Ok(PackageManifest {
            id,
            name,
            version,
            package_type,
            description,
            readme: self.readme,
            keywords: self.keywords,
            categories: self.categories,
            license: self.license.unwrap_or_else(default_license),
            repository: self.repository,
            documentation: self.documentation,
            homepage: self.homepage,
            author,
            contributors: self.contributors,
            maintainers: self.maintainers,
            signatures: self.signatures,
            lineage: self.lineage,
            audit: self.audit,
            dependencies: self.dependencies,
            optional_dependencies: self.optional_dependencies,
            dashflow_version: self
                .dashflow_version
                .unwrap_or_else(default_dashflow_version),
            permissions: self.permissions,
            provides: self.provides,
            metadata: self.metadata,
        })
    }
}

/// Package author information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    /// Author name
    pub name: String,
    /// Author email
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Key ID for signature verification
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_id: Option<String>,
    /// Author URL/website
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl Author {
    /// Create a new author.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            email: None,
            key_id: None,
            url: None,
        }
    }

    /// Set the email.
    #[must_use]
    pub fn with_email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(email.into());
        self
    }

    /// Set the key ID.
    #[must_use]
    pub fn with_key_id(mut self, key_id: impl Into<String>) -> Self {
        self.key_id = Some(key_id.into());
        self
    }

    /// Set the URL.
    #[must_use]
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }
}

/// Package contributor information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contributor {
    /// Contributor name
    pub name: String,
    /// Contributor email
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// What they contributed
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contributions: Vec<String>,
}

impl Contributor {
    /// Create a new contributor.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            email: None,
            contributions: Vec::new(),
        }
    }

    /// Set the email.
    #[must_use]
    pub fn with_email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(email.into());
        self
    }

    /// Add contributions.
    pub fn with_contributions(
        mut self,
        contributions: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.contributions
            .extend(contributions.into_iter().map(Into::into));
        self
    }
}

/// Package maintainer information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Maintainer {
    /// Maintainer name
    pub name: String,
    /// Maintainer email
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Key ID for publishing
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_id: Option<String>,
}

impl Maintainer {
    /// Create a new maintainer.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            email: None,
            key_id: None,
        }
    }

    /// Set the email.
    #[must_use]
    pub fn with_email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(email.into());
        self
    }

    /// Set the key ID.
    #[must_use]
    pub fn with_key_id(mut self, key_id: impl Into<String>) -> Self {
        self.key_id = Some(key_id.into());
        self
    }
}

/// Package dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    /// Package ID
    pub package: PackageId,
    /// Version requirement
    pub version: VersionReq,
}

impl Dependency {
    /// Create a new dependency.
    pub fn new(package: PackageId, version: VersionReq) -> Self {
        Self { package, version }
    }
}

/// Optional dependency (feature-gated).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionalDependency {
    /// Feature name that enables this dependency
    pub feature: String,
    /// Package ID
    pub package: PackageId,
    /// Version requirement
    pub version: VersionReq,
}

impl OptionalDependency {
    /// Create a new optional dependency.
    pub fn new(feature: impl Into<String>, package: PackageId, version: VersionReq) -> Self {
        Self {
            feature: feature.into(),
            package,
            version,
        }
    }
}

/// Required permission for a package.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    /// Network access
    Network,
    /// Filesystem read access
    FilesystemRead,
    /// Filesystem write access
    FilesystemWrite,
    /// LLM API access
    Llm,
    /// Environment variable access
    Environment,
    /// Process spawning
    Process,
    /// Custom permission
    Custom(String),
}

impl Permission {
    /// Get string representation.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Network => "network",
            Self::FilesystemRead => "filesystem_read",
            Self::FilesystemWrite => "filesystem_write",
            Self::Llm => "llm",
            Self::Environment => "environment",
            Self::Process => "process",
            Self::Custom(s) => s,
        }
    }
}

/// Capability provided by a package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    /// Capability name (e.g., "sentiment-analysis", "code-review")
    pub name: String,
    /// Description of what this capability does
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Semantic tags for discovery
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl Capability {
    /// Create a new capability.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            tags: Vec::new(),
        }
    }

    /// Set the description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add tags.
    #[must_use]
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags.extend(tags.into_iter().map(Into::into));
        self
    }
}

/// Security audit status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditStatus {
    /// Audit status level
    pub status: AuditStatusLevel,
    /// Who performed the audit
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auditor: Option<String>,
    /// When the audit was performed (ISO 8601)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// URL to the audit report
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_url: Option<String>,
    /// Audit notes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// Audit status level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditStatusLevel {
    /// Not audited
    None,
    /// Audit in progress
    Pending,
    /// Audit completed, package verified
    Verified,
    /// Audit found issues
    Failed,
}

impl AuditStatus {
    /// Create a verified audit status.
    pub fn verified(auditor: impl Into<String>, date: impl Into<String>) -> Self {
        Self {
            status: AuditStatusLevel::Verified,
            auditor: Some(auditor.into()),
            date: Some(date.into()),
            report_url: None,
            notes: None,
        }
    }

    /// Set the report URL.
    #[must_use]
    pub fn with_report_url(mut self, url: impl Into<String>) -> Self {
        self.report_url = Some(url.into());
        self
    }
}

/// Package lineage (derivation history).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lineage {
    /// Original package this was derived from
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_from: Option<PackageRef>,
    /// Chain of derivations
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chain: Vec<DerivationStep>,
    /// Has the lineage been verified?
    #[serde(default)]
    pub verified: bool,
}

impl Lineage {
    /// Create a new lineage from a source package.
    pub fn from(source: PackageRef) -> Self {
        Self {
            derived_from: Some(source),
            chain: Vec::new(),
            verified: false,
        }
    }

    /// Add a derivation step.
    #[must_use]
    pub fn with_step(mut self, step: DerivationStep) -> Self {
        self.chain.push(step);
        self
    }

    /// Mark as verified.
    pub fn verified(mut self) -> Self {
        self.verified = true;
        self
    }
}

/// A step in the derivation chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivationStep {
    /// Package at this step
    pub package: PackageRef,
    /// What changed
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<String>,
    /// Why it changed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Who made the change
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// When (ISO 8601)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

impl DerivationStep {
    /// Create a new derivation step.
    pub fn new(package: PackageRef) -> Self {
        Self {
            package,
            changes: Vec::new(),
            reason: None,
            author: None,
            timestamp: None,
        }
    }

    /// Add changes.
    #[must_use]
    pub fn with_changes(mut self, changes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.changes.extend(changes.into_iter().map(Into::into));
        self
    }

    /// Set the reason.
    #[must_use]
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

/// Reference to a specific package version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageRef {
    /// Package ID
    pub id: PackageId,
    /// Specific version
    pub version: Version,
    /// Content hash (for verification)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

impl PackageRef {
    /// Create a new package reference.
    pub fn new(id: PackageId, version: Version) -> Self {
        Self {
            id,
            version,
            hash: None,
        }
    }

    /// Set the content hash.
    #[must_use]
    pub fn with_hash(mut self, hash: impl Into<String>) -> Self {
        self.hash = Some(hash.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_builder() {
        let manifest = PackageManifest::builder()
            .id("dashflow", "sentiment-analysis")
            .name("Sentiment Analysis")
            .version(1, 2, 0)
            .package_type(PackageType::NodeLibrary)
            .description("Production-grade sentiment analysis nodes")
            .author(Author::new("DashFlow Team").with_email("packages@dashswarm.com"))
            .keywords(["sentiment", "nlp", "text-analysis"])
            .categories(["nlp", "analysis"])
            .license("Apache-2.0")
            .repository("https://github.com/dashflow/sentiment-analysis")
            .build();

        assert_eq!(manifest.id.to_string(), "dashflow/sentiment-analysis");
        assert_eq!(manifest.name, "Sentiment Analysis");
        assert_eq!(manifest.version, Version::new(1, 2, 0));
        assert_eq!(manifest.package_type, PackageType::NodeLibrary);
        assert_eq!(manifest.keywords.len(), 3);
        assert!(!manifest.is_signed());
        assert!(!manifest.is_audited());
    }

    #[test]
    fn test_manifest_try_build_missing_field() {
        let result = PackageManifest::builder().id("test", "pkg").try_build();

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("required"));
    }

    #[test]
    fn test_author() {
        let author = Author::new("Test Author")
            .with_email("test@example.com")
            .with_key_id("key-123")
            .with_url("https://example.com");

        assert_eq!(author.name, "Test Author");
        assert_eq!(author.email, Some("test@example.com".to_string()));
        assert_eq!(author.key_id, Some("key-123".to_string()));
    }

    #[test]
    fn test_contributor() {
        let contributor = Contributor::new("Jane Dev")
            .with_email("jane@example.com")
            .with_contributions(["Added feature X", "Fixed bug Y"]);

        assert_eq!(contributor.name, "Jane Dev");
        assert_eq!(contributor.contributions.len(), 2);
    }

    #[test]
    fn test_dependency() {
        let dep = Dependency::new(
            PackageId::new("dashflow", "core"),
            VersionReq::at_least(Version::new(1, 0, 0)),
        );

        assert_eq!(dep.package.to_string(), "dashflow/core");
        assert!(dep.version.matches(&Version::new(1, 5, 0)));
    }

    #[test]
    fn test_capability() {
        let cap = Capability::new("sentiment-analysis")
            .with_description("Analyze sentiment in text")
            .with_tags(["nlp", "text", "emotion"]);

        assert_eq!(cap.name, "sentiment-analysis");
        assert_eq!(cap.tags.len(), 3);
    }

    #[test]
    fn test_audit_status() {
        let audit = AuditStatus::verified("security-team", "2025-12-10")
            .with_report_url("https://example.com/audit/123");

        assert_eq!(audit.status, AuditStatusLevel::Verified);
        assert_eq!(audit.auditor, Some("security-team".to_string()));
    }

    #[test]
    fn test_lineage() {
        let source = PackageRef::new(
            PackageId::new("community", "basic-sentiment"),
            Version::new(0, 5, 0),
        );

        let lineage = Lineage::from(source)
            .with_step(
                DerivationStep::new(PackageRef::new(
                    PackageId::new("mycompany", "sentiment-enhanced"),
                    Version::new(1, 0, 0),
                ))
                .with_changes(["Added GPU support"])
                .with_reason("Performance improvement"),
            )
            .verified();

        assert!(lineage.derived_from.is_some());
        assert_eq!(lineage.chain.len(), 1);
        assert!(lineage.verified);
    }

    #[test]
    fn test_permission_variants() {
        assert_eq!(Permission::Network.as_str(), "network");
        assert_eq!(Permission::FilesystemRead.as_str(), "filesystem_read");
        assert_eq!(
            Permission::Custom("custom_perm".to_string()).as_str(),
            "custom_perm"
        );
    }

    #[test]
    fn test_manifest_with_dependencies() {
        let manifest = PackageManifest::builder()
            .id("test", "pkg")
            .name("Test Package")
            .version(1, 0, 0)
            .package_type(PackageType::ToolPack)
            .description("A test package")
            .author(Author::new("Test"))
            .dependency(
                PackageId::new("dashflow", "core"),
                VersionReq::caret(Version::new(1, 11, 0)),
            )
            .build();

        assert_eq!(manifest.dependencies.len(), 1);
        assert_eq!(manifest.dependency_ids().len(), 1);
    }

    #[test]
    fn test_manifest_content_hash() {
        let manifest = PackageManifest::builder()
            .id("test", "pkg")
            .name("Test")
            .version(1, 0, 0)
            .package_type(PackageType::ToolPack)
            .description("Test")
            .author(Author::new("Test"))
            .build();

        let hash = manifest.content_hash(HashAlgorithm::Sha256).unwrap();
        assert!(hash.starts_with("sha256:"));
        // SHA256 produces 64 hex chars + "sha256:" prefix = 71 chars
        assert!(hash.len() >= 71);

        // Test other algorithms
        let hash384 = manifest.content_hash(HashAlgorithm::Sha384).unwrap();
        assert!(hash384.starts_with("sha384:"));

        let hash512 = manifest.content_hash(HashAlgorithm::Sha512).unwrap();
        assert!(hash512.starts_with("sha512:"));

        let blake3_hash = manifest.content_hash(HashAlgorithm::Blake3).unwrap();
        assert!(blake3_hash.starts_with("blake3:"));
    }
}
