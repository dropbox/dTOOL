//! Cryptographic signatures for packages.
//!
//! Uses Ed25519 for signing and verification.
//! Every package must be signed by a registered key.

use crate::content_hash::ContentHash;
use crate::error::{RegistryError, Result};
use crate::package::PackageManifest;
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};

/// A public key for signature verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicKey {
    /// Key ID (hex-encoded first 8 bytes of public key).
    pub key_id: String,
    /// The full public key bytes.
    #[serde(with = "hex_bytes")]
    pub bytes: [u8; 32],
    /// Owner name/identifier.
    pub owner: String,
    /// When this key was registered.
    pub registered_at: DateTime<Utc>,
    /// Is this key currently active?
    pub active: bool,
}

impl Default for PublicKey {
    fn default() -> Self {
        Self {
            key_id: String::new(),
            bytes: [0u8; 32],
            owner: String::new(),
            registered_at: Utc::now(),
            active: false,
        }
    }
}

impl PublicKey {
    /// Create a PublicKey from raw bytes.
    pub fn from_bytes(bytes: [u8; 32], owner: String) -> Self {
        let key_id = hex::encode(&bytes[..8]);
        Self {
            key_id,
            bytes,
            owner,
            registered_at: Utc::now(),
            active: true,
        }
    }

    /// Get the verifying key for signature verification.
    pub fn verifying_key(&self) -> Result<VerifyingKey> {
        VerifyingKey::from_bytes(&self.bytes)
            .map_err(|e| RegistryError::InvalidSignature(e.to_string()))
    }
}

/// A key pair for signing packages.
pub struct KeyPair {
    /// The signing key (private).
    signing_key: SigningKey,
    /// The public key.
    pub public_key: PublicKey,
}

impl KeyPair {
    /// Generate a new random key pair.
    pub fn generate(owner: String) -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let public_key = PublicKey::from_bytes(verifying_key.to_bytes(), owner);

        Self {
            signing_key,
            public_key,
        }
    }

    /// Sign data and return a signature.
    pub fn sign(&self, data: &[u8]) -> Signature {
        let sig = self.signing_key.sign(data);
        Signature {
            key_id: self.public_key.key_id.clone(),
            signature: sig.to_bytes().to_vec(),
            timestamp: Utc::now(),
        }
    }

    /// Sign a package (manifest + content hash).
    pub fn sign_package(&self, manifest: &PackageManifest, hash: &ContentHash) -> Signature {
        let signed_content = create_signed_content(manifest, hash);
        self.sign(&signed_content)
    }

    /// Get the key ID.
    pub fn key_id(&self) -> &str {
        &self.public_key.key_id
    }
}

/// A signature over package content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signature {
    /// ID of the signing key.
    pub key_id: String,
    /// The signature bytes.
    #[serde(with = "hex_vec")]
    pub signature: Vec<u8>,
    /// When this signature was created.
    pub timestamp: DateTime<Utc>,
}

impl Signature {
    /// Verify this signature against data using a public key.
    pub fn verify(&self, data: &[u8], public_key: &PublicKey) -> Result<bool> {
        let verifying_key = public_key.verifying_key()?;

        let sig_bytes: [u8; 64] = self.signature.clone().try_into().map_err(|v: Vec<u8>| {
            RegistryError::InvalidSignature(format!("signature must be 64 bytes, got {}", v.len()))
        })?;

        let signature = ed25519_dalek::Signature::from_bytes(&sig_bytes);

        Ok(verifying_key.verify(data, &signature).is_ok())
    }

    /// Verify this signature against a package.
    pub fn verify_package(
        &self,
        manifest: &PackageManifest,
        hash: &ContentHash,
        public_key: &PublicKey,
    ) -> Result<bool> {
        let signed_content = create_signed_content(manifest, hash);
        self.verify(&signed_content, public_key)
    }
}

/// A signed package - package data with signatures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedPackage {
    /// Content hash of the package.
    pub hash: ContentHash,
    /// Package manifest.
    pub manifest: PackageManifest,
    /// Signatures from trusted keys.
    pub signatures: Vec<Signature>,
}

impl SignedPackage {
    /// Create a new signed package.
    pub fn new(manifest: PackageManifest, hash: ContentHash) -> Self {
        Self {
            hash,
            manifest,
            signatures: Vec::new(),
        }
    }

    /// Add a signature.
    pub fn add_signature(&mut self, signature: Signature) {
        self.signatures.push(signature);
    }

    /// Check if this package has any valid signatures.
    pub fn has_valid_signature(&self, keys: &[PublicKey]) -> bool {
        for sig in &self.signatures {
            for key in keys {
                if key.key_id == sig.key_id
                    && sig
                        .verify_package(&self.manifest, &self.hash, key)
                        .unwrap_or(false)
                {
                    return true;
                }
            }
        }
        false
    }
}

/// Create the canonical content to be signed for a package.
fn create_signed_content(manifest: &PackageManifest, hash: &ContentHash) -> Vec<u8> {
    // Sign: name + version + hash
    // This binds the signature to specific content
    format!(
        "dashflow-package:{}:{}:{}",
        manifest.name, manifest.version, hash
    )
    .into_bytes()
}

/// Serde helper for hex-encoded fixed-size byte arrays.
mod hex_bytes {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S, T>(bytes: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: AsRef<[u8]>,
    {
        hex::encode(bytes.as_ref()).serialize(serializer)
    }

    pub fn deserialize<'de, D, const N: usize>(deserializer: D) -> Result<[u8; N], D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let bytes = hex::decode(&s).map_err(serde::de::Error::custom)?;
        let actual_len = bytes.len();
        bytes
            .try_into()
            .map_err(|_| serde::de::Error::custom(format!("wrong byte length: expected {N}, got {actual_len}")))
    }
}

/// Serde helper for hex-encoded `Vec<u8>`.
mod hex_vec {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        hex::encode(bytes).serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        hex::decode(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_manifest() -> PackageManifest {
        PackageManifest::builder()
            .name("test-package")
            .version("1.0.0")
            .description("Test")
            .build()
            .unwrap()
    }

    #[test]
    fn test_keypair_generation() {
        let keypair = KeyPair::generate("test-owner".to_string());
        assert!(!keypair.key_id().is_empty());
        assert_eq!(keypair.public_key.owner, "test-owner");
    }

    #[test]
    fn test_sign_and_verify() {
        let keypair = KeyPair::generate("test".to_string());
        let data = b"hello world";

        let signature = keypair.sign(data);
        assert!(signature.verify(data, &keypair.public_key).unwrap());
        assert!(!signature.verify(b"different", &keypair.public_key).unwrap());
    }

    #[test]
    fn test_sign_package() {
        let keypair = KeyPair::generate("publisher".to_string());
        let manifest = test_manifest();
        let hash = ContentHash::from_bytes(b"package-content");

        let signature = keypair.sign_package(&manifest, &hash);
        assert!(signature
            .verify_package(&manifest, &hash, &keypair.public_key)
            .unwrap());

        // Different hash should fail
        let different_hash = ContentHash::from_bytes(b"different-content");
        assert!(!signature
            .verify_package(&manifest, &different_hash, &keypair.public_key)
            .unwrap());
    }

    #[test]
    fn test_signed_package() {
        let keypair = KeyPair::generate("publisher".to_string());
        let manifest = test_manifest();
        let hash = ContentHash::from_bytes(b"content");

        let mut signed = SignedPackage::new(manifest.clone(), hash.clone());
        let signature = keypair.sign_package(&manifest, &hash);
        signed.add_signature(signature);

        assert!(signed.has_valid_signature(&[keypair.public_key]));
    }

    #[test]
    fn test_public_key_serialization() {
        let keypair = KeyPair::generate("test".to_string());
        let json = serde_json::to_string(&keypair.public_key).unwrap();
        let parsed: PublicKey = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.key_id, keypair.public_key.key_id);
        assert_eq!(parsed.bytes, keypair.public_key.bytes);
    }
}
