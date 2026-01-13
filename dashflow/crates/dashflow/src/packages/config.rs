// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Registry configuration and source management.
//!
//! This module provides configuration for the multi-registry system, allowing
//! users to specify multiple package sources in priority order:
//!
//! 1. Local registry (~/.dashflow/packages/)
//! 2. Team/git registries
//! 3. HTTP registries (including dashswarm.com)
//! 4. Colony peers (P2P sharing)
//!
//! # Example Configuration
//!
//! ```toml
//! # ~/.dashflow/registries.toml
//!
//! [[registry]]
//! type = "local"
//! path = "~/.dashflow/packages"
//! writable = true
//!
//! [[registry]]
//! type = "git"
//! url = "git@github.com:mycompany/dashflow-packages.git"
//! branch = "main"
//!
//! [[registry]]
//! type = "http"
//! url = "https://registry.dashswarm.com"
//! official = true
//!
//! [trust]
//! required_signatures = ["dashflow-official"]
//! reject_vulnerable = true
//! ```

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

use super::client::HttpAuth;
use super::dashswarm::DASHSWARM_DEFAULT_URL;
use super::types::TrustLevel;

/// Complete registry configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryConfig {
    /// Registries in priority order (first match wins)
    #[serde(rename = "registry", default)]
    pub registries: Vec<RegistrySource>,
    /// Trust settings
    #[serde(default)]
    pub trust: TrustConfig,
    /// Cache settings
    #[serde(default)]
    pub cache: CacheConfig,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            registries: vec![
                // 1. Local packages (highest priority)
                RegistrySource::Local {
                    path: PathBuf::from("~/.dashflow/packages"),
                    writable: true,
                },
                // 2. Official central registry
                RegistrySource::Http {
                    url: DASHSWARM_DEFAULT_URL.to_string(),
                    auth: None,
                    official: true,
                    trust: None,
                },
                // 3. Colony peers (fallback)
                RegistrySource::Colony {
                    fallback_only: true,
                },
            ],
            trust: TrustConfig::default(),
            cache: CacheConfig::default(),
        }
    }
}

impl RegistryConfig {
    /// Load configuration from a file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Io(format!("Failed to read {}: {}", path.display(), e)))?;

        toml::from_str(&content)
            .map_err(|e| ConfigError::Parse(format!("Failed to parse {}: {}", path.display(), e)))
    }

    /// Load configuration from the default location (~/.dashflow/registries.toml).
    pub fn load_default() -> Result<Self, ConfigError> {
        let config_path = Self::default_config_path()
            .ok_or_else(|| ConfigError::Io("Could not determine home directory".to_string()))?;

        if config_path.exists() {
            Self::load(&config_path)
        } else {
            Ok(Self::default())
        }
    }

    /// Get the default configuration file path.
    pub fn default_config_path() -> Option<PathBuf> {
        dirs::home_dir().map(|home| home.join(".dashflow").join("registries.toml"))
    }

    /// Save configuration to a file.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), ConfigError> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| ConfigError::Parse(format!("Failed to serialize config: {}", e)))?;

        std::fs::write(path.as_ref(), content)
            .map_err(|e| ConfigError::Io(format!("Failed to write config: {}", e)))
    }

    /// Add a registry source.
    pub fn add_registry(&mut self, source: RegistrySource) {
        self.registries.push(source);
    }

    /// Insert a registry at a specific priority (0 = highest).
    pub fn insert_registry(&mut self, index: usize, source: RegistrySource) {
        self.registries
            .insert(index.min(self.registries.len()), source);
    }

    /// Remove a registry by index.
    pub fn remove_registry(&mut self, index: usize) -> Option<RegistrySource> {
        if index < self.registries.len() {
            Some(self.registries.remove(index))
        } else {
            None
        }
    }

    /// Get HTTP registries.
    pub fn http_registries(&self) -> impl Iterator<Item = &RegistrySource> {
        self.registries
            .iter()
            .filter(|r| matches!(r, RegistrySource::Http { .. }))
    }

    /// Get the official registry URL (if any).
    pub fn official_registry_url(&self) -> Option<&str> {
        for source in &self.registries {
            if let RegistrySource::Http {
                url,
                official: true,
                ..
            } = source
            {
                return Some(url);
            }
        }
        None
    }

    /// Get local registries.
    pub fn local_registries(&self) -> impl Iterator<Item = &RegistrySource> {
        self.registries
            .iter()
            .filter(|r| matches!(r, RegistrySource::Local { .. }))
    }

    /// Check if colony sources are enabled.
    pub fn has_colony_source(&self) -> bool {
        self.registries
            .iter()
            .any(|r| matches!(r, RegistrySource::Colony { .. }))
    }
}

/// A package registry source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RegistrySource {
    /// Local filesystem registry
    Local {
        /// Path to the registry directory
        path: PathBuf,
        /// Can packages be installed/removed from this registry?
        #[serde(default = "default_true")]
        writable: bool,
    },

    /// Git-based registry
    Git {
        /// Git repository URL
        url: String,
        /// Branch to use (default: main)
        branch: Option<String>,
        /// Authentication
        auth: Option<GitAuth>,
    },

    /// HTTP registry (like dashswarm.com)
    Http {
        /// Base URL of the registry API
        url: String,
        /// Authentication
        auth: Option<HttpAuth>,
        /// Is this an official registry?
        #[serde(default)]
        official: bool,
        /// Minimum trust level for packages from this registry
        trust: Option<TrustLevel>,
    },

    /// Colony peer (discover packages from other apps)
    Colony {
        /// Only use if package not found elsewhere
        #[serde(default = "default_true")]
        fallback_only: bool,
    },
}

fn default_true() -> bool {
    true
}

impl RegistrySource {
    /// Create a local registry source.
    pub fn local(path: impl Into<PathBuf>) -> Self {
        Self::Local {
            path: path.into(),
            writable: true,
        }
    }

    /// Create an HTTP registry source.
    pub fn http(url: impl Into<String>) -> Self {
        Self::Http {
            url: url.into(),
            auth: None,
            official: false,
            trust: None,
        }
    }

    /// Create the official registry source.
    pub fn official() -> Self {
        Self::Http {
            url: DASHSWARM_DEFAULT_URL.to_string(),
            auth: None,
            official: true,
            trust: None,
        }
    }

    /// Create a git registry source.
    pub fn git(url: impl Into<String>) -> Self {
        Self::Git {
            url: url.into(),
            branch: None,
            auth: None,
        }
    }

    /// Create a colony registry source.
    pub fn colony() -> Self {
        Self::Colony {
            fallback_only: true,
        }
    }

    /// Get the display name for this source.
    pub fn display_name(&self) -> String {
        match self {
            Self::Local { path, .. } => format!("local:{}", path.display()),
            Self::Git { url, branch, .. } => {
                if let Some(b) = branch {
                    format!("git:{}#{}", url, b)
                } else {
                    format!("git:{}", url)
                }
            }
            Self::Http { url, official, .. } => {
                if *official {
                    format!("official:{}", url)
                } else {
                    format!("http:{}", url)
                }
            }
            Self::Colony { .. } => "colony".to_string(),
        }
    }
}

/// Git authentication methods.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum GitAuth {
    /// SSH key authentication
    Ssh {
        /// Path to private key (default: ~/.ssh/id_ed25519)
        key_path: Option<PathBuf>,
    },
    /// HTTP basic authentication.
    Basic {
        /// Username for authentication.
        username: String,
        /// Password for authentication.
        password: String,
    },
    /// Personal access token.
    Token {
        /// The access token value.
        token: String,
    },
}

/// Trust configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrustConfig {
    /// Required signatures for installation (key IDs)
    #[serde(default)]
    pub required_signatures: RequiredSignatures,
    /// Allow unsigned packages from these namespaces
    #[serde(default)]
    pub allow_unsigned: Vec<String>,
    /// Reject packages with security advisories
    #[serde(default = "default_true")]
    pub reject_vulnerable: bool,
    /// Minimum trust level for packages
    #[serde(default)]
    pub minimum_trust: Option<TrustLevel>,
}

impl TrustConfig {
    /// Create a new trust configuration with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the required signatures policy.
    #[must_use]
    pub fn with_required_signatures(mut self, required: RequiredSignatures) -> Self {
        self.required_signatures = required;
        self
    }

    /// Set the namespaces that allow unsigned packages.
    #[must_use]
    pub fn with_allow_unsigned(mut self, namespaces: Vec<String>) -> Self {
        self.allow_unsigned = namespaces;
        self
    }

    /// Add a namespace that allows unsigned packages.
    #[must_use]
    pub fn allow_unsigned_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.allow_unsigned.push(namespace.into());
        self
    }

    /// Set whether to reject packages with security advisories.
    #[must_use]
    pub fn with_reject_vulnerable(mut self, reject: bool) -> Self {
        self.reject_vulnerable = reject;
        self
    }

    /// Set the minimum trust level for packages.
    #[must_use]
    pub fn with_minimum_trust(mut self, level: Option<TrustLevel>) -> Self {
        self.minimum_trust = level;
        self
    }

    /// Check if a namespace allows unsigned packages.
    pub fn allows_unsigned(&self, namespace: &str) -> bool {
        for pattern in &self.allow_unsigned {
            if pattern == "*" || pattern == namespace {
                return true;
            }
            if let Some(prefix) = pattern.strip_suffix("/*") {
                if namespace.starts_with(prefix) || namespace == prefix {
                    return true;
                }
            }
        }
        false
    }
}

/// Required signature configuration.
///
/// Can be one of:
/// - `"none"` - No signatures required
/// - `"any"` - Any signature from a known key
/// - `"official"` - Must have official DashFlow signature
/// - `["key1", "key2"]` - Specific key IDs required
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum RequiredSignatures {
    /// No signatures required
    #[default]
    None,
    /// Any signature from a known key
    Any,
    /// Must have official signature
    Official,
    /// Specific key IDs required
    Keys(Vec<String>),
}

impl RequiredSignatures {
    /// Check if signatures are required.
    pub fn is_required(&self) -> bool {
        !matches!(self, Self::None)
    }
}

impl Serialize for RequiredSignatures {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::None => serializer.serialize_str("none"),
            Self::Any => serializer.serialize_str("any"),
            Self::Official => serializer.serialize_str("official"),
            Self::Keys(keys) => keys.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for RequiredSignatures {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, Visitor};

        struct RequiredSignaturesVisitor;

        impl<'de> Visitor<'de> for RequiredSignaturesVisitor {
            type Value = RequiredSignatures;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("\"none\", \"any\", \"official\", or a list of key IDs")
            }

            fn visit_str<E>(self, value: &str) -> Result<RequiredSignatures, E>
            where
                E: de::Error,
            {
                match value.to_lowercase().as_str() {
                    "none" => Ok(RequiredSignatures::None),
                    "any" => Ok(RequiredSignatures::Any),
                    "official" => Ok(RequiredSignatures::Official),
                    _ => Err(de::Error::unknown_variant(
                        value,
                        &["none", "any", "official"],
                    )),
                }
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<RequiredSignatures, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let mut keys = Vec::new();
                while let Some(key) = seq.next_element()? {
                    keys.push(key);
                }
                Ok(RequiredSignatures::Keys(keys))
            }
        }

        deserializer.deserialize_any(RequiredSignaturesVisitor)
    }
}

/// Cache configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Cache directory (default: ~/.dashflow/cache)
    pub path: Option<PathBuf>,
    /// Maximum cache size in megabytes
    #[serde(default = "default_cache_size")]
    pub max_size_mb: u64,
    /// Cache package metadata for this duration (seconds)
    #[serde(default = "default_metadata_ttl")]
    pub metadata_ttl_secs: u64,
    /// Enable offline mode (only use cached packages)
    #[serde(default)]
    pub offline: bool,
}

fn default_cache_size() -> u64 {
    5000 // 5 GB
}

fn default_metadata_ttl() -> u64 {
    3600 // 1 hour
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            path: None,
            max_size_mb: default_cache_size(),
            metadata_ttl_secs: default_metadata_ttl(),
            offline: false,
        }
    }
}

impl CacheConfig {
    /// Get the cache directory path.
    pub fn cache_path(&self) -> Option<PathBuf> {
        self.path
            .clone()
            .or_else(|| dirs::home_dir().map(|home| home.join(".dashflow").join("cache")))
    }
}

/// Configuration errors.
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// IO error
    #[error("IO error: {0}")]
    Io(String),
    /// Parse error
    #[error("Parse error: {0}")]
    Parse(String),
    /// Validation error
    #[error("Validation error: {0}")]
    Validation(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_registry_config_default() {
        let config = RegistryConfig::default();

        // Should have local, http, and colony sources
        assert_eq!(config.registries.len(), 3);
        assert!(matches!(config.registries[0], RegistrySource::Local { .. }));
        assert!(matches!(
            config.registries[1],
            RegistrySource::Http { official: true, .. }
        ));
        assert!(matches!(
            config.registries[2],
            RegistrySource::Colony { .. }
        ));
    }

    #[test]
    fn test_registry_config_load_save() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("registries.toml");

        let config = RegistryConfig::default();
        config.save(&config_path).unwrap();

        let loaded = RegistryConfig::load(&config_path).unwrap();
        assert_eq!(loaded.registries.len(), config.registries.len());
    }

    #[test]
    fn test_registry_source_local() {
        let source = RegistrySource::local("/tmp/packages");
        assert!(matches!(
            source,
            RegistrySource::Local { writable: true, .. }
        ));
        assert!(source.display_name().contains("/tmp/packages"));
    }

    #[test]
    fn test_registry_source_http() {
        let source = RegistrySource::http("https://example.com");
        assert!(matches!(
            source,
            RegistrySource::Http {
                official: false,
                ..
            }
        ));
        assert!(source.display_name().contains("example.com"));
    }

    #[test]
    fn test_registry_source_official() {
        let source = RegistrySource::official();
        assert!(matches!(
            source,
            RegistrySource::Http { official: true, .. }
        ));
        assert!(source.display_name().starts_with("official:"));
    }

    #[test]
    fn test_registry_source_git() {
        let source = RegistrySource::git("git@github.com:test/repo.git");
        assert!(matches!(source, RegistrySource::Git { .. }));
        assert!(source.display_name().contains("github.com"));
    }

    #[test]
    fn test_registry_config_add_registry() {
        let mut config = RegistryConfig::default();
        let initial_count = config.registries.len();

        config.add_registry(RegistrySource::http("https://custom.com"));
        assert_eq!(config.registries.len(), initial_count + 1);
    }

    #[test]
    fn test_registry_config_insert_registry() {
        let mut config = RegistryConfig::default();

        config.insert_registry(0, RegistrySource::http("https://priority.com"));
        assert!(
            matches!(&config.registries[0], RegistrySource::Http { url, .. } if url.contains("priority"))
        );
    }

    #[test]
    fn test_registry_config_http_registries() {
        let config = RegistryConfig::default();
        let http_count = config.http_registries().count();
        assert_eq!(http_count, 1); // Just the official registry
    }

    #[test]
    fn test_registry_config_official_url() {
        let config = RegistryConfig::default();
        assert_eq!(
            config.official_registry_url(),
            Some("https://registry.dashswarm.com")
        );
    }

    #[test]
    fn test_trust_config_allows_unsigned() {
        let config = TrustConfig {
            allow_unsigned: vec!["local/*".to_string(), "dev".to_string()],
            ..Default::default()
        };

        assert!(config.allows_unsigned("local"));
        assert!(config.allows_unsigned("local/test"));
        assert!(config.allows_unsigned("dev"));
        assert!(!config.allows_unsigned("community"));
    }

    #[test]
    fn test_trust_config_wildcard() {
        let config = TrustConfig {
            allow_unsigned: vec!["*".to_string()],
            ..Default::default()
        };

        assert!(config.allows_unsigned("anything"));
    }

    #[test]
    fn test_required_signatures() {
        assert!(!RequiredSignatures::None.is_required());
        assert!(RequiredSignatures::Any.is_required());
        assert!(RequiredSignatures::Official.is_required());
        assert!(RequiredSignatures::Keys(vec!["key1".to_string()]).is_required());
    }

    #[test]
    fn test_cache_config_default() {
        let config = CacheConfig::default();
        assert_eq!(config.max_size_mb, 5000);
        assert_eq!(config.metadata_ttl_secs, 3600);
        assert!(!config.offline);
    }

    #[test]
    fn test_cache_config_cache_path() {
        let config = CacheConfig::default();
        let path = config.cache_path();
        assert!(path.is_some());
        assert!(path.unwrap().to_string_lossy().contains(".dashflow"));
    }

    #[test]
    fn test_config_error_display() {
        let err = ConfigError::Io("test error".to_string());
        assert!(err.to_string().contains("IO error"));

        let err = ConfigError::Parse("parse error".to_string());
        assert!(err.to_string().contains("Parse error"));
    }

    #[test]
    fn test_parse_config_toml() {
        let toml_str = r#"
[[registry]]
type = "local"
path = "~/.dashflow/packages"
writable = true

[[registry]]
type = "http"
url = "https://registry.dashswarm.com"
official = true

[trust]
reject_vulnerable = true

[cache]
max_size_mb = 1000
"#;

        let config: RegistryConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.registries.len(), 2);
        assert!(config.trust.reject_vulnerable);
        assert_eq!(config.cache.max_size_mb, 1000);
    }

    // =========================================================================
    // TrustConfig Builder Pattern Tests
    // =========================================================================

    #[test]
    fn test_trust_config_new() {
        let config = TrustConfig::new();
        assert_eq!(config.required_signatures, RequiredSignatures::None);
        assert!(config.allow_unsigned.is_empty());
        // Note: Default for bool is false; serde uses default_true only for deserialization
        assert!(!config.reject_vulnerable);
        assert!(config.minimum_trust.is_none());
    }

    #[test]
    fn test_trust_config_builder_required_signatures() {
        let config = TrustConfig::new().with_required_signatures(RequiredSignatures::Official);
        assert_eq!(config.required_signatures, RequiredSignatures::Official);
    }

    #[test]
    fn test_trust_config_builder_allow_unsigned() {
        let config = TrustConfig::new()
            .with_allow_unsigned(vec!["local/*".to_string(), "dev".to_string()]);
        assert_eq!(config.allow_unsigned.len(), 2);
        assert!(config.allows_unsigned("local/test"));
        assert!(config.allows_unsigned("dev"));
    }

    #[test]
    fn test_trust_config_builder_allow_unsigned_namespace() {
        let config = TrustConfig::new()
            .allow_unsigned_namespace("local/*")
            .allow_unsigned_namespace("dev");
        assert_eq!(config.allow_unsigned.len(), 2);
        assert!(config.allows_unsigned("local/test"));
        assert!(config.allows_unsigned("dev"));
    }

    #[test]
    fn test_trust_config_builder_reject_vulnerable() {
        let config = TrustConfig::new().with_reject_vulnerable(false);
        assert!(!config.reject_vulnerable);
    }

    #[test]
    fn test_trust_config_builder_minimum_trust() {
        let config = TrustConfig::new().with_minimum_trust(Some(TrustLevel::Verified));
        assert_eq!(config.minimum_trust, Some(TrustLevel::Verified));
    }

    #[test]
    fn test_trust_config_builder_chaining() {
        let config = TrustConfig::new()
            .with_required_signatures(RequiredSignatures::Any)
            .allow_unsigned_namespace("local/*")
            .with_reject_vulnerable(true)
            .with_minimum_trust(Some(TrustLevel::Community));

        assert_eq!(config.required_signatures, RequiredSignatures::Any);
        assert!(config.allows_unsigned("local/test"));
        assert!(config.reject_vulnerable);
        assert_eq!(config.minimum_trust, Some(TrustLevel::Community));
    }

    #[test]
    fn test_trust_config_builder_with_keys() {
        let config = TrustConfig::new().with_required_signatures(RequiredSignatures::Keys(vec![
            "key1".to_string(),
            "key2".to_string(),
        ]));
        assert!(config.required_signatures.is_required());
        if let RequiredSignatures::Keys(keys) = &config.required_signatures {
            assert_eq!(keys.len(), 2);
        } else {
            panic!("Expected Keys variant");
        }
    }
}
