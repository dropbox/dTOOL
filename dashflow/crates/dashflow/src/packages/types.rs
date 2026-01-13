// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Core types for the DashFlow package ecosystem.
//!
//! These types form the foundation of the package system and are used across
//! all registry implementations.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Package identifier with namespace.
///
/// Format: `namespace/name` (e.g., "dashflow/sentiment-analysis")
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PackageId {
    /// Namespace (e.g., "dashflow", "mycompany", "community")
    namespace: String,
    /// Package name (e.g., "sentiment-analysis")
    name: String,
}

impl PackageId {
    /// Create a new package ID.
    pub fn new(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            name: name.into(),
        }
    }

    /// Get the namespace.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Get the package name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Parse from string format "namespace/name".
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.splitn(2, '/').collect();
        if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            Some(Self::new(parts[0], parts[1]))
        } else {
            None
        }
    }
}

impl fmt::Display for PackageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.namespace, self.name)
    }
}

impl FromStr for PackageId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("Invalid package ID: {}", s))
    }
}

/// Package types supported by the ecosystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackageType {
    /// Pre-built graph architectures for common use cases
    GraphTemplate,
    /// Collections of specialized nodes
    NodeLibrary,
    /// Sets of tools for specific domains
    ToolPack,
    /// Storage implementations for state persistence
    CheckpointerBackend,
    /// LLM provider integrations
    ModelConnector,
    /// Curated prompt collections
    PromptLibrary,
}

impl PackageType {
    /// Get the string representation of the package type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::GraphTemplate => "graph-template",
            Self::NodeLibrary => "node-library",
            Self::ToolPack => "tool-pack",
            Self::CheckpointerBackend => "checkpointer-backend",
            Self::ModelConnector => "model-connector",
            Self::PromptLibrary => "prompt-library",
        }
    }

    /// Parse from string.
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s {
            "graph-template" => Some(Self::GraphTemplate),
            "node-library" => Some(Self::NodeLibrary),
            "tool-pack" => Some(Self::ToolPack),
            "checkpointer-backend" => Some(Self::CheckpointerBackend),
            "model-connector" => Some(Self::ModelConnector),
            "prompt-library" => Some(Self::PromptLibrary),
            _ => None,
        }
    }
}

impl fmt::Display for PackageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for PackageType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str_opt(s).ok_or_else(|| format!("Unknown package type: {}", s))
    }
}

/// Semantic version following semver 2.0.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Version {
    /// Major version (breaking changes)
    pub major: u32,
    /// Minor version (new features, backward compatible)
    pub minor: u32,
    /// Patch version (bug fixes)
    pub patch: u32,
    /// Pre-release label (e.g., "alpha", "beta.1")
    pub pre: Option<String>,
    /// Build metadata (e.g., "build.123")
    pub build: Option<String>,
}

impl Version {
    /// Create a new version.
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
            pre: None,
            build: None,
        }
    }

    /// Create a version with pre-release label.
    #[must_use]
    pub fn with_pre(mut self, pre: impl Into<String>) -> Self {
        self.pre = Some(pre.into());
        self
    }

    /// Create a version with build metadata.
    #[must_use]
    pub fn with_build(mut self, build: impl Into<String>) -> Self {
        self.build = Some(build.into());
        self
    }

    /// Parse from string (e.g., "1.2.3", "1.2.3-beta", "1.2.3+build.456").
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();

        // Split off build metadata first
        let (version_pre, build) = if let Some(idx) = s.find('+') {
            (&s[..idx], Some(s[idx + 1..].to_string()))
        } else {
            (s, None)
        };

        // Split off pre-release
        let (version, pre) = if let Some(idx) = version_pre.find('-') {
            (
                &version_pre[..idx],
                Some(version_pre[idx + 1..].to_string()),
            )
        } else {
            (version_pre, None)
        };

        // Parse major.minor.patch
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() != 3 {
            return None;
        }

        let major = parts[0].parse().ok()?;
        let minor = parts[1].parse().ok()?;
        let patch = parts[2].parse().ok()?;

        Some(Self {
            major,
            minor,
            patch,
            pre,
            build,
        })
    }

    /// Check if this is a pre-release version.
    pub fn is_prerelease(&self) -> bool {
        self.pre.is_some()
    }

    /// Compare versions (ignoring build metadata per semver).
    pub fn cmp_semver(&self, other: &Self) -> std::cmp::Ordering {
        match self.major.cmp(&other.major) {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        match self.minor.cmp(&other.minor) {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        match self.patch.cmp(&other.patch) {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        // Pre-release versions have lower precedence
        match (&self.pre, &other.pre) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(_), None) => std::cmp::Ordering::Less,
            (Some(a), Some(b)) => a.cmp(b),
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(pre) = &self.pre {
            write!(f, "-{}", pre)?;
        }
        if let Some(build) = &self.build {
            write!(f, "+{}", build)?;
        }
        Ok(())
    }
}

impl FromStr for Version {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("Invalid version: {}", s))
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(std::cmp::Ord::cmp(self, other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.cmp_semver(other)
    }
}

/// Version requirement for dependencies.
///
/// Supports common version constraints:
/// - Exact: `=1.2.3`
/// - Greater than: `>1.2.3`
/// - Greater or equal: `>=1.2.3`
/// - Less than: `<1.2.3`
/// - Less or equal: `<=1.2.3`
/// - Caret (compatible): `^1.2.3` (default, allows patch and minor updates)
/// - Tilde (compatible): `~1.2.3` (allows patch updates)
/// - Wildcard: `*`, `1.*`, `1.2.*`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionReq {
    /// The constraint operator
    pub op: VersionOp,
    /// The base version for comparison
    pub version: Version,
}

/// Version constraint operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersionOp {
    /// Exact match: `=1.2.3`
    Exact,
    /// Greater than: `>1.2.3`
    Greater,
    /// Greater or equal: `>=1.2.3`
    GreaterEq,
    /// Less than: `<1.2.3`
    Less,
    /// Less or equal: `<=1.2.3`
    LessEq,
    /// Caret (compatible updates): `^1.2.3`
    Caret,
    /// Tilde (patch updates only): `~1.2.3`
    Tilde,
    /// Any version: `*`
    Any,
}

impl VersionReq {
    /// Create a version requirement.
    pub fn new(op: VersionOp, version: Version) -> Self {
        Self { op, version }
    }

    /// Create an exact version requirement.
    pub fn exact(version: Version) -> Self {
        Self::new(VersionOp::Exact, version)
    }

    /// Create a caret (compatible) version requirement.
    pub fn caret(version: Version) -> Self {
        Self::new(VersionOp::Caret, version)
    }

    /// Create a "greater or equal" requirement.
    pub fn at_least(version: Version) -> Self {
        Self::new(VersionOp::GreaterEq, version)
    }

    /// Create an "any version" requirement.
    pub fn any() -> Self {
        Self::new(VersionOp::Any, Version::new(0, 0, 0))
    }

    /// Check if a version matches this requirement.
    pub fn matches(&self, version: &Version) -> bool {
        match self.op {
            VersionOp::Any => true,
            VersionOp::Exact => version.cmp_semver(&self.version) == std::cmp::Ordering::Equal,
            VersionOp::Greater => version.cmp_semver(&self.version) == std::cmp::Ordering::Greater,
            VersionOp::GreaterEq => version.cmp_semver(&self.version) != std::cmp::Ordering::Less,
            VersionOp::Less => version.cmp_semver(&self.version) == std::cmp::Ordering::Less,
            VersionOp::LessEq => version.cmp_semver(&self.version) != std::cmp::Ordering::Greater,
            VersionOp::Caret => {
                // ^1.2.3 allows 1.2.3 <= x < 2.0.0 (for major >= 1)
                // ^0.2.3 allows 0.2.3 <= x < 0.3.0 (for major == 0)
                // ^0.0.3 allows 0.0.3 <= x < 0.0.4 (for major == 0, minor == 0)
                if version.cmp_semver(&self.version) == std::cmp::Ordering::Less {
                    return false;
                }
                if self.version.major == 0 {
                    if self.version.minor == 0 {
                        version.major == 0
                            && version.minor == 0
                            && version.patch == self.version.patch
                    } else {
                        version.major == 0 && version.minor == self.version.minor
                    }
                } else {
                    version.major == self.version.major
                }
            }
            VersionOp::Tilde => {
                // ~1.2.3 allows 1.2.3 <= x < 1.3.0
                if version.cmp_semver(&self.version) == std::cmp::Ordering::Less {
                    return false;
                }
                version.major == self.version.major && version.minor == self.version.minor
            }
        }
    }

    /// Parse from string.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();

        if s == "*" {
            return Some(Self::any());
        }

        let (op, version_str) = if let Some(rest) = s.strip_prefix(">=") {
            (VersionOp::GreaterEq, rest)
        } else if let Some(rest) = s.strip_prefix("<=") {
            (VersionOp::LessEq, rest)
        } else if let Some(rest) = s.strip_prefix('>') {
            (VersionOp::Greater, rest)
        } else if let Some(rest) = s.strip_prefix('<') {
            (VersionOp::Less, rest)
        } else if let Some(rest) = s.strip_prefix('=') {
            (VersionOp::Exact, rest)
        } else if let Some(rest) = s.strip_prefix('^') {
            (VersionOp::Caret, rest)
        } else if let Some(rest) = s.strip_prefix('~') {
            (VersionOp::Tilde, rest)
        } else {
            // Default to caret
            (VersionOp::Caret, s)
        };

        let version = Version::parse(version_str.trim())?;
        Some(Self::new(op, version))
    }
}

impl fmt::Display for VersionReq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.op {
            VersionOp::Any => write!(f, "*"),
            VersionOp::Exact => write!(f, "={}", self.version),
            VersionOp::Greater => write!(f, ">{}", self.version),
            VersionOp::GreaterEq => write!(f, ">={}", self.version),
            VersionOp::Less => write!(f, "<{}", self.version),
            VersionOp::LessEq => write!(f, "<={}", self.version),
            VersionOp::Caret => write!(f, "^{}", self.version),
            VersionOp::Tilde => write!(f, "~{}", self.version),
        }
    }
}

impl FromStr for VersionReq {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("Invalid version requirement: {}", s))
    }
}

/// Cryptographic signature on a package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signature {
    /// Key identifier (references a known public key)
    pub key_id: String,
    /// Signature algorithm
    pub algorithm: SignatureAlgorithm,
    /// The signature bytes (base64 encoded)
    pub signature: String,
    /// What was signed
    pub signed_content: SignedContent,
    /// Timestamp of signature (ISO 8601)
    pub timestamp: String,
}

impl Signature {
    /// Create a new signature.
    pub fn new(
        key_id: impl Into<String>,
        algorithm: SignatureAlgorithm,
        signature: impl Into<String>,
        signed_content: SignedContent,
        timestamp: impl Into<String>,
    ) -> Self {
        Self {
            key_id: key_id.into(),
            algorithm,
            signature: signature.into(),
            signed_content,
            timestamp: timestamp.into(),
        }
    }
}

/// Signature algorithm.
///
/// Serializes to snake_case (e.g., "rsa_pss_4096", "ecdsa_p256").
/// Accepts legacy lowercase format for backwards compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureAlgorithm {
    /// Ed25519 (recommended)
    Ed25519,
    /// RSA-PSS with 4096-bit key
    #[serde(alias = "rsapss4096")]
    RsaPss4096,
    /// ECDSA with P-256 curve
    #[serde(alias = "ecdsap256")]
    EcdsaP256,
}

/// What content was signed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignedContent {
    /// Signed the manifest hash.
    ManifestHash {
        /// The hash value that was signed.
        hash: String,
        /// Algorithm used to compute the hash.
        algorithm: HashAlgorithm,
    },
    /// Signed the full package tarball.
    PackageHash {
        /// The hash value that was signed.
        hash: String,
        /// Algorithm used to compute the hash.
        algorithm: HashAlgorithm,
    },
    /// Signed both manifest and package.
    Both {
        /// Hash of the manifest.
        manifest_hash: String,
        /// Hash of the package tarball.
        package_hash: String,
        /// Algorithm used to compute both hashes.
        algorithm: HashAlgorithm,
    },
}

/// Hash algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HashAlgorithm {
    /// SHA-256 (recommended)
    Sha256,
    /// SHA-384
    Sha384,
    /// SHA-512
    Sha512,
    /// BLAKE3 (fast, secure)
    Blake3,
}

/// Trust level for keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrustLevel {
    /// Local/development key (lowest trust)
    Local,
    /// Community contributor
    Community,
    /// Verified organization
    Verified,
    /// Official DashFlow key (highest trust)
    Official,
}

impl TrustLevel {
    /// Get string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Community => "community",
            Self::Verified => "verified",
            Self::Official => "official",
        }
    }
}

impl fmt::Display for TrustLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_id_new() {
        let id = PackageId::new("dashflow", "sentiment-analysis");
        assert_eq!(id.namespace(), "dashflow");
        assert_eq!(id.name(), "sentiment-analysis");
        assert_eq!(id.to_string(), "dashflow/sentiment-analysis");
    }

    #[test]
    fn test_package_id_parse() {
        let id = PackageId::parse("mycompany/my-package").unwrap();
        assert_eq!(id.namespace(), "mycompany");
        assert_eq!(id.name(), "my-package");

        assert!(PackageId::parse("invalid").is_none());
        assert!(PackageId::parse("/nonamespace").is_none());
        assert!(PackageId::parse("noname/").is_none());
    }

    #[test]
    fn test_package_type() {
        assert_eq!(PackageType::GraphTemplate.as_str(), "graph-template");
        assert_eq!(
            PackageType::from_str_opt("node-library"),
            Some(PackageType::NodeLibrary)
        );
        assert_eq!(PackageType::from_str_opt("unknown"), None);
    }

    #[test]
    fn test_version_new() {
        let v = Version::new(1, 2, 3);
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.to_string(), "1.2.3");
    }

    #[test]
    fn test_version_with_pre() {
        let v = Version::new(1, 0, 0).with_pre("beta.1");
        assert_eq!(v.to_string(), "1.0.0-beta.1");
        assert!(v.is_prerelease());
    }

    #[test]
    fn test_version_with_build() {
        let v = Version::new(1, 0, 0).with_build("build.456");
        assert_eq!(v.to_string(), "1.0.0+build.456");
        assert!(!v.is_prerelease());
    }

    #[test]
    fn test_version_parse() {
        let v = Version::parse("1.2.3").unwrap();
        assert_eq!(v, Version::new(1, 2, 3));

        let v = Version::parse("2.0.0-alpha").unwrap();
        assert_eq!(v.pre, Some("alpha".to_string()));

        let v = Version::parse("3.1.4+build.789").unwrap();
        assert_eq!(v.build, Some("build.789".to_string()));

        let v = Version::parse("1.0.0-rc.1+sha.abc123").unwrap();
        assert_eq!(v.pre, Some("rc.1".to_string()));
        assert_eq!(v.build, Some("sha.abc123".to_string()));

        assert!(Version::parse("invalid").is_none());
        assert!(Version::parse("1.2").is_none());
    }

    #[test]
    fn test_version_comparison() {
        assert!(Version::new(1, 0, 0) < Version::new(2, 0, 0));
        assert!(Version::new(1, 1, 0) < Version::new(1, 2, 0));
        assert!(Version::new(1, 1, 1) < Version::new(1, 1, 2));

        // Pre-release has lower precedence
        assert!(Version::new(1, 0, 0).with_pre("alpha") < Version::new(1, 0, 0));
        assert!(Version::new(1, 0, 0).with_pre("alpha") < Version::new(1, 0, 0).with_pre("beta"));
    }

    #[test]
    fn test_version_req_exact() {
        let req = VersionReq::exact(Version::new(1, 2, 3));
        assert!(req.matches(&Version::new(1, 2, 3)));
        assert!(!req.matches(&Version::new(1, 2, 4)));
        assert!(!req.matches(&Version::new(1, 3, 0)));
    }

    #[test]
    fn test_version_req_caret() {
        let req = VersionReq::caret(Version::new(1, 2, 3));
        assert!(req.matches(&Version::new(1, 2, 3)));
        assert!(req.matches(&Version::new(1, 2, 4)));
        assert!(req.matches(&Version::new(1, 9, 0)));
        assert!(!req.matches(&Version::new(2, 0, 0)));
        assert!(!req.matches(&Version::new(1, 2, 2)));
    }

    #[test]
    fn test_version_req_caret_zero_major() {
        let req = VersionReq::caret(Version::new(0, 2, 3));
        assert!(req.matches(&Version::new(0, 2, 3)));
        assert!(req.matches(&Version::new(0, 2, 9)));
        assert!(!req.matches(&Version::new(0, 3, 0)));
    }

    #[test]
    fn test_version_req_tilde() {
        let req = VersionReq::parse("~1.2.3").unwrap();
        assert!(req.matches(&Version::new(1, 2, 3)));
        assert!(req.matches(&Version::new(1, 2, 9)));
        assert!(!req.matches(&Version::new(1, 3, 0)));
    }

    #[test]
    fn test_version_req_at_least() {
        let req = VersionReq::at_least(Version::new(1, 0, 0));
        assert!(req.matches(&Version::new(1, 0, 0)));
        assert!(req.matches(&Version::new(2, 0, 0)));
        assert!(!req.matches(&Version::new(0, 9, 0)));
    }

    #[test]
    fn test_version_req_any() {
        let req = VersionReq::any();
        assert!(req.matches(&Version::new(0, 0, 1)));
        assert!(req.matches(&Version::new(99, 99, 99)));
    }

    #[test]
    fn test_version_req_parse() {
        assert_eq!(VersionReq::parse("*").unwrap().op, VersionOp::Any);
        assert_eq!(VersionReq::parse("=1.0.0").unwrap().op, VersionOp::Exact);
        assert_eq!(VersionReq::parse(">1.0.0").unwrap().op, VersionOp::Greater);
        assert_eq!(
            VersionReq::parse(">=1.0.0").unwrap().op,
            VersionOp::GreaterEq
        );
        assert_eq!(VersionReq::parse("<1.0.0").unwrap().op, VersionOp::Less);
        assert_eq!(VersionReq::parse("<=1.0.0").unwrap().op, VersionOp::LessEq);
        assert_eq!(VersionReq::parse("^1.0.0").unwrap().op, VersionOp::Caret);
        assert_eq!(VersionReq::parse("~1.0.0").unwrap().op, VersionOp::Tilde);
        // Default is caret
        assert_eq!(VersionReq::parse("1.0.0").unwrap().op, VersionOp::Caret);
    }

    #[test]
    fn test_trust_level_ordering() {
        assert!(TrustLevel::Local < TrustLevel::Community);
        assert!(TrustLevel::Community < TrustLevel::Verified);
        assert!(TrustLevel::Verified < TrustLevel::Official);
    }

    #[test]
    fn test_signature() {
        let sig = Signature::new(
            "dashflow-official-2024",
            SignatureAlgorithm::Ed25519,
            "base64signature==",
            SignedContent::ManifestHash {
                hash: "abc123".to_string(),
                algorithm: HashAlgorithm::Sha256,
            },
            "2025-12-10T00:00:00Z",
        );
        assert_eq!(sig.key_id, "dashflow-official-2024");
        assert_eq!(sig.algorithm, SignatureAlgorithm::Ed25519);
    }
}
