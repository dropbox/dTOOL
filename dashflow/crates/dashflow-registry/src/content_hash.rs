//! Content-addressed hashing for packages.
//!
//! Every package is stored and retrieved by its SHA-256 content hash.
//! This provides:
//! - Deduplication: Same content = same hash = stored once
//! - Verification: Download and verify hash matches
//! - Immutability: Content at a hash never changes
//! - Distribution: Any peer with the hash can serve it

use crate::error::{RegistryError, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// A SHA-256 content hash.
///
/// Used to uniquely identify package content in the registry.
/// Format: `sha256:<64 hex characters>`
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    /// Create a content hash from raw bytes.
    ///
    /// # Example
    /// ```
    /// use dashflow_registry::ContentHash;
    ///
    /// let data = b"hello world";
    /// let hash = ContentHash::from_bytes(data);
    /// assert!(hash.to_string().starts_with("sha256:"));
    /// ```
    pub fn from_bytes(data: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        Self(result.into())
    }

    /// Create a content hash from a hex string (with or without sha256: prefix).
    ///
    /// # Example
    /// ```
    /// use dashflow_registry::ContentHash;
    ///
    /// let hex = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
    /// let hash = ContentHash::from_hex(hex).unwrap();
    /// assert_eq!(hash.to_hex(), hex);
    /// ```
    /// Alias for from_hex - parse from string representation.
    pub fn from_string(s: &str) -> Result<Self> {
        Self::from_hex(s)
    }

    pub fn from_hex(s: &str) -> Result<Self> {
        let hex_str = s.strip_prefix("sha256:").unwrap_or(s);

        if hex_str.len() != 64 {
            return Err(RegistryError::InvalidContentHash(format!(
                "expected 64 hex characters, got {}",
                hex_str.len()
            )));
        }

        let bytes = hex::decode(hex_str)
            .map_err(|e| RegistryError::InvalidContentHash(format!("invalid hex: {}", e)))?;

        let array: [u8; 32] = bytes.try_into().map_err(|v: Vec<u8>| {
            RegistryError::InvalidContentHash(format!(
                "hash must be exactly 32 bytes, got {}",
                v.len()
            ))
        })?;

        Ok(Self(array))
    }

    /// Get the raw 32-byte hash.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Get the hex-encoded hash without prefix.
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Verify that data matches this hash.
    ///
    /// # Example
    /// ```
    /// use dashflow_registry::ContentHash;
    ///
    /// let data = b"hello world";
    /// let hash = ContentHash::from_bytes(data);
    /// assert!(hash.verify(data));
    /// assert!(!hash.verify(b"different data"));
    /// ```
    pub fn verify(&self, data: &[u8]) -> bool {
        let computed = Self::from_bytes(data);
        computed == *self
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sha256:{}", hex::encode(self.0))
    }
}

impl fmt::Debug for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContentHash({})", self)
    }
}

impl std::str::FromStr for ContentHash {
    type Err = RegistryError;

    fn from_str(s: &str) -> Result<Self> {
        Self::from_hex(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_hash_from_bytes() {
        let data = b"hello world";
        let hash = ContentHash::from_bytes(data);

        // SHA-256 of "hello world"
        assert_eq!(
            hash.to_string(),
            "sha256:b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_content_hash_from_hex() {
        let hex = "sha256:b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        let hash = ContentHash::from_hex(hex).unwrap();

        assert_eq!(hash.to_string(), hex);
    }

    #[test]
    fn test_content_hash_from_hex_without_prefix() {
        let hex = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        let hash = ContentHash::from_hex(hex).unwrap();

        assert_eq!(hash.to_hex(), hex);
    }

    #[test]
    fn test_content_hash_verify() {
        let data = b"hello world";
        let hash = ContentHash::from_bytes(data);

        assert!(hash.verify(data));
        assert!(!hash.verify(b"hello worlD")); // Different case
        assert!(!hash.verify(b"different"));
    }

    #[test]
    fn test_content_hash_invalid_hex() {
        let result = ContentHash::from_hex("not-valid-hex");
        assert!(result.is_err());

        let result = ContentHash::from_hex("sha256:abc"); // Too short
        assert!(result.is_err());
    }

    #[test]
    fn test_content_hash_equality() {
        let hash1 = ContentHash::from_bytes(b"test");
        let hash2 = ContentHash::from_bytes(b"test");
        let hash3 = ContentHash::from_bytes(b"different");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_content_hash_serialization() {
        let hash = ContentHash::from_bytes(b"test");
        let json = serde_json::to_string(&hash).unwrap();
        let parsed: ContentHash = serde_json::from_str(&json).unwrap();

        assert_eq!(hash, parsed);
    }
}
