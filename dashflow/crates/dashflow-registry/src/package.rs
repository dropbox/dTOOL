//! Package manifest and metadata types.
//!
//! Defines the structure of packages in the registry, including:
//! - Package manifest (name, version, dependencies)
//! - Capabilities (what the package provides)
//! - Lineage (derivation chain for trust)
//! - Trust levels

use crate::content_hash::ContentHash;
use chrono::{DateTime, Utc};
use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Type of package in the registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum PackageType {
    /// A DashFlow agent or node.
    Agent,
    /// A tool for agents to use.
    Tool,
    /// A prompt template.
    Prompt,
    /// An embedding model or configuration.
    Embedding,
    /// A retrieval pipeline.
    Retrieval,
    /// A complete application.
    Application,
    /// A library/utility.
    #[default]
    Library,
    /// Other package type.
    Other(String),
}

/// Trust level for packages and signatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum TrustLevel {
    /// Unknown/unverified.
    #[default]
    Unknown = 0,
    /// Community verified (signed by any registered key).
    Community = 1,
    /// Verified organization.
    Organization = 2,
    /// Official DashFlow package.
    Official = 3,
}

/// A capability that a package provides.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Capability {
    /// Capability name (e.g., "sentiment_analysis", "code_generation").
    pub name: String,
    /// Optional version requirement for this capability.
    pub version: Option<String>,
    /// Description of what this capability provides.
    pub description: Option<String>,
}

impl Capability {
    /// Create a new capability with just a name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: None,
            description: None,
        }
    }

    /// Create a capability with description.
    pub fn with_description(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: None,
            description: Some(description.into()),
        }
    }
}

/// Package manifest - the core metadata for a package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManifest {
    /// Package name (e.g., "sentiment-analyzer").
    pub name: String,

    /// Semantic version.
    pub version: Version,

    /// Namespace/organization (e.g., "dashflow", "acme-corp").
    #[serde(default)]
    pub namespace: Option<String>,

    /// Short description.
    pub description: String,

    /// Package type.
    #[serde(default)]
    pub package_type: PackageType,

    /// Keywords for search.
    #[serde(default)]
    pub keywords: Vec<String>,

    /// Capabilities this package provides.
    #[serde(default)]
    pub provides: Vec<Capability>,

    /// Capabilities this package requires.
    #[serde(default)]
    pub requires: Vec<Capability>,

    /// Dependencies on other packages.
    #[serde(default)]
    pub dependencies: Vec<Dependency>,

    /// Authors.
    #[serde(default)]
    pub authors: Vec<String>,

    /// License identifier (SPDX).
    pub license: Option<String>,

    /// Homepage URL.
    pub homepage: Option<String>,

    /// Repository URL.
    pub repository: Option<String>,

    /// Documentation URL.
    pub documentation: Option<String>,

    /// README content (for display).
    pub readme: Option<String>,

    /// Minimum DashFlow version required.
    pub min_dashflow_version: Option<String>,
}

impl PackageManifest {
    /// Create a new manifest builder.
    pub fn builder() -> PackageManifestBuilder {
        PackageManifestBuilder::default()
    }

    /// Get the fully qualified name (namespace/name).
    pub fn qualified_name(&self) -> String {
        match &self.namespace {
            Some(ns) => format!("{}/{}", ns, self.name),
            None => self.name.clone(),
        }
    }
}

/// Builder for PackageManifest.
#[derive(Debug, Default)]
pub struct PackageManifestBuilder {
    name: Option<String>,
    version: Option<Version>,
    namespace: Option<String>,
    description: Option<String>,
    package_type: PackageType,
    keywords: Vec<String>,
    provides: Vec<Capability>,
    requires: Vec<Capability>,
    dependencies: Vec<Dependency>,
    authors: Vec<String>,
    license: Option<String>,
    homepage: Option<String>,
    repository: Option<String>,
    documentation: Option<String>,
    readme: Option<String>,
    min_dashflow_version: Option<String>,
}

impl PackageManifestBuilder {
    /// Set the package name (required).
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the version (required).
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = Version::parse(&version.into()).ok();
        self
    }

    /// Set the namespace.
    pub fn namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    /// Set the description (required).
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the package type.
    pub fn package_type(mut self, package_type: PackageType) -> Self {
        self.package_type = package_type;
        self
    }

    /// Add a keyword.
    pub fn keyword(mut self, keyword: impl Into<String>) -> Self {
        self.keywords.push(keyword.into());
        self
    }

    /// Add multiple keywords.
    pub fn keywords(mut self, keywords: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.keywords.extend(keywords.into_iter().map(Into::into));
        self
    }

    /// Add a capability this package provides.
    pub fn provides(mut self, capability: Capability) -> Self {
        self.provides.push(capability);
        self
    }

    /// Add a capability this package requires.
    pub fn requires(mut self, capability: Capability) -> Self {
        self.requires.push(capability);
        self
    }

    /// Add a dependency.
    pub fn dependency(mut self, dependency: Dependency) -> Self {
        self.dependencies.push(dependency);
        self
    }

    /// Add an author.
    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.authors.push(author.into());
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

    /// Set the README content.
    pub fn readme(mut self, readme: impl Into<String>) -> Self {
        self.readme = Some(readme.into());
        self
    }

    /// Build the manifest.
    pub fn build(self) -> Result<PackageManifest, &'static str> {
        Ok(PackageManifest {
            name: self.name.ok_or("name is required")?,
            version: self.version.ok_or("version is required")?,
            namespace: self.namespace,
            description: self.description.ok_or("description is required")?,
            package_type: self.package_type,
            keywords: self.keywords,
            provides: self.provides,
            requires: self.requires,
            dependencies: self.dependencies,
            authors: self.authors,
            license: self.license,
            homepage: self.homepage,
            repository: self.repository,
            documentation: self.documentation,
            readme: self.readme,
            min_dashflow_version: self.min_dashflow_version,
        })
    }
}

/// A dependency on another package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    /// Package name.
    pub name: String,
    /// Version requirement (semver).
    pub version_req: String,
    /// Optional: specific content hash to pin to.
    pub hash: Option<ContentHash>,
    /// Is this an optional dependency?
    #[serde(default)]
    pub optional: bool,
}

impl Dependency {
    /// Create a new dependency.
    pub fn new(name: impl Into<String>, version_req: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version_req: version_req.into(),
            hash: None,
            optional: false,
        }
    }

    /// Create an optional dependency.
    pub fn optional(name: impl Into<String>, version_req: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version_req: version_req.into(),
            hash: None,
            optional: true,
        }
    }
}

/// Full package info including registry metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    /// Content hash of the package.
    pub hash: ContentHash,

    /// Package manifest.
    pub manifest: PackageManifest,

    /// When this version was published.
    pub published_at: DateTime<Utc>,

    /// Who published it (key ID).
    pub publisher_key_id: String,

    /// Download count.
    pub downloads: u64,

    /// Trust level.
    pub trust_level: TrustLevel,

    /// Lineage (derivation chain).
    pub lineage: Option<Lineage>,

    /// Is this version yanked?
    #[serde(default)]
    pub yanked: bool,
}

/// Lineage/derivation chain for a package.
///
/// Tracks the history of how a package was created/modified,
/// enabling trust propagation and audit trails.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lineage {
    /// The original package this was derived from (if any).
    pub derived_from: Option<ContentHash>,

    /// Chain of derivation steps.
    pub chain: Vec<LineageStep>,
}

/// A single step in a lineage chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageStep {
    /// Unique ID for this step.
    pub id: Uuid,

    /// Source package hash.
    pub source_hash: ContentHash,

    /// Result package hash.
    pub result_hash: ContentHash,

    /// Type of derivation.
    pub derivation_type: DerivationType,

    /// Who performed this derivation.
    pub actor: String,

    /// When this derivation was performed.
    pub timestamp: DateTime<Utc>,

    /// Signature of this step.
    pub signature: String,
}

/// Type of derivation in a lineage chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DerivationType {
    /// Original creation.
    Original,
    /// Fork/copy.
    Fork,
    /// Bug fix.
    BugFix,
    /// Enhancement/improvement.
    Enhancement,
    /// Optimization.
    Optimization,
    /// AI-generated improvement.
    AiImprovement,
    /// Merge of multiple packages.
    Merge,
    /// Other.
    Other(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_manifest_builder() {
        let manifest = PackageManifest::builder()
            .name("test-package")
            .version("1.0.0")
            .description("A test package")
            .namespace("test-org")
            .keyword("test")
            .keyword("example")
            .license("MIT")
            .build()
            .unwrap();

        assert_eq!(manifest.name, "test-package");
        assert_eq!(manifest.version.to_string(), "1.0.0");
        assert_eq!(manifest.qualified_name(), "test-org/test-package");
        assert_eq!(manifest.keywords.len(), 2);
    }

    #[test]
    fn test_package_manifest_builder_missing_fields() {
        let result = PackageManifest::builder().name("test").build();
        assert!(result.is_err());
    }

    #[test]
    fn test_capability() {
        let cap = Capability::new("sentiment_analysis");
        assert_eq!(cap.name, "sentiment_analysis");
        assert!(cap.description.is_none());

        let cap = Capability::with_description("code_gen", "Generate code from prompts");
        assert!(cap.description.is_some());
    }

    #[test]
    fn test_dependency() {
        let dep = Dependency::new("some-lib", "^1.0");
        assert!(!dep.optional);

        let dep = Dependency::optional("optional-lib", ">=2.0");
        assert!(dep.optional);
    }

    #[test]
    fn test_trust_level_ordering() {
        assert!(TrustLevel::Official > TrustLevel::Organization);
        assert!(TrustLevel::Organization > TrustLevel::Community);
        assert!(TrustLevel::Community > TrustLevel::Unknown);
    }

    #[test]
    fn test_package_type_serialization() {
        let agent = PackageType::Agent;
        let json = serde_json::to_string(&agent).unwrap();
        assert_eq!(json, "\"agent\"");

        let parsed: PackageType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, PackageType::Agent);
    }
}
