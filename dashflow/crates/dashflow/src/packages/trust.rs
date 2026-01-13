// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # Package Trust System
//!
//! Cryptographic signature verification and key management for the DashFlow package ecosystem.
//!
//! This module provides:
//! - **Key management**: Store and retrieve trusted public keys
//! - **Signature verification**: Verify Ed25519, RSA-PSS, and ECDSA signatures
//! - **Package signing**: Create signatures for packages
//! - **Hash computation**: SHA-256/384/512 and BLAKE3 hashing
//!
//! ## Security Note
//!
//! The RSA implementation (rsa crate v0.9.x) has a known timing vulnerability
//! (RUSTSEC-2023-0071 Marvin attack). Use Ed25519 or ECDSA when possible.
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::packages::{KeyStore, TrustedKey, TrustLevel, PackageVerifier};
//!
//! // Create a key store
//! let mut store = KeyStore::new();
//!
//! // Add a trusted key
//! let key = TrustedKey::new(
//!     "dashflow-official-2025",
//!     ed25519_public_key_pem,
//!     "DashFlow Team",
//!     TrustLevel::Official,
//! );
//! store.add_key(key)?;
//!
//! // Create a verifier
//! let verifier = PackageVerifier::new(&store);
//!
//! // Verify a package signature
//! let result = verifier.verify_signature(&signature, &content)?;
//! ```

use crate::packages::{HashAlgorithm, Signature, SignatureAlgorithm, SignedContent, TrustLevel};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha384, Sha512};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors from the trust system.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TrustError {
    /// The requested key was not found in the key store.
    #[error("Key not found: {0}")]
    KeyNotFound(String),

    /// The key exists but has passed its expiration date.
    #[error("Key expired: {0}")]
    KeyExpired(String),

    /// Cryptographic signature verification failed.
    #[error("Invalid signature: {0}")]
    InvalidSignature(String),

    /// Computed hash does not match the expected hash.
    #[error("Hash mismatch: expected {expected}, got {actual}")]
    HashMismatch {
        /// Expected hash value from the signature.
        expected: String,
        /// Actual computed hash of the content.
        actual: String,
    },

    /// The specified algorithm is not supported.
    #[error("Unsupported algorithm: {0}")]
    UnsupportedAlgorithm(String),

    /// Failed to parse or encode a cryptographic key.
    #[error("Key encoding error: {0}")]
    KeyEncodingError(String),

    /// Signing operation failed.
    #[error("Signing error: {0}")]
    SigningError(String),

    /// I/O error during key store operations.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Failed to serialize or deserialize key data.
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// The signing key's trust level is below the required threshold.
    #[error("Insufficient trust level: required {required:?}, got {actual:?}")]
    InsufficientTrust {
        /// Minimum trust level required for this operation.
        required: TrustLevel,
        /// Actual trust level of the signing key.
        actual: TrustLevel,
    },

    /// The key has been revoked and cannot be used for verification.
    #[error("Key revoked: {0}")]
    KeyRevoked(String),
}

/// Result type for trust operations.
pub type TrustResult<T> = Result<T, TrustError>;

/// A trusted public key for signature verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedKey {
    /// Unique key identifier
    pub key_id: String,
    /// Public key in PEM format
    pub public_key_pem: String,
    /// Key algorithm
    pub algorithm: SignatureAlgorithm,
    /// Who owns this key
    pub owner: String,
    /// Trust level
    pub trust_level: TrustLevel,
    /// Key creation timestamp (ISO 8601)
    pub created: String,
    /// Key expiration timestamp (ISO 8601), if any
    pub expires: Option<String>,
    /// Whether the key has been revoked
    pub revoked: bool,
    /// Revocation reason, if revoked
    pub revocation_reason: Option<String>,
    /// Optional fingerprint (SHA-256 of public key bytes)
    pub fingerprint: Option<String>,
}

impl TrustedKey {
    /// Create a new Ed25519 trusted key.
    pub fn new_ed25519(
        key_id: impl Into<String>,
        public_key_pem: impl Into<String>,
        owner: impl Into<String>,
        trust_level: TrustLevel,
    ) -> Self {
        Self {
            key_id: key_id.into(),
            public_key_pem: public_key_pem.into(),
            algorithm: SignatureAlgorithm::Ed25519,
            owner: owner.into(),
            trust_level,
            created: chrono::Utc::now().to_rfc3339(),
            expires: None,
            revoked: false,
            revocation_reason: None,
            fingerprint: None,
        }
    }

    /// Create a new ECDSA P-256 trusted key.
    pub fn new_ecdsa_p256(
        key_id: impl Into<String>,
        public_key_pem: impl Into<String>,
        owner: impl Into<String>,
        trust_level: TrustLevel,
    ) -> Self {
        Self {
            key_id: key_id.into(),
            public_key_pem: public_key_pem.into(),
            algorithm: SignatureAlgorithm::EcdsaP256,
            owner: owner.into(),
            trust_level,
            created: chrono::Utc::now().to_rfc3339(),
            expires: None,
            revoked: false,
            revocation_reason: None,
            fingerprint: None,
        }
    }

    /// Create a new RSA-PSS 4096-bit trusted key.
    pub fn new_rsa_pss(
        key_id: impl Into<String>,
        public_key_pem: impl Into<String>,
        owner: impl Into<String>,
        trust_level: TrustLevel,
    ) -> Self {
        Self {
            key_id: key_id.into(),
            public_key_pem: public_key_pem.into(),
            algorithm: SignatureAlgorithm::RsaPss4096,
            owner: owner.into(),
            trust_level,
            created: chrono::Utc::now().to_rfc3339(),
            expires: None,
            revoked: false,
            revocation_reason: None,
            fingerprint: None,
        }
    }

    /// Set expiration date.
    #[must_use]
    pub fn with_expiration(mut self, expires: impl Into<String>) -> Self {
        self.expires = Some(expires.into());
        self
    }

    /// Set fingerprint.
    #[must_use]
    pub fn with_fingerprint(mut self, fingerprint: impl Into<String>) -> Self {
        self.fingerprint = Some(fingerprint.into());
        self
    }

    /// Check if the key is currently valid (not expired, not revoked).
    pub fn is_valid(&self) -> bool {
        if self.revoked {
            return false;
        }
        if let Some(expires) = &self.expires {
            if let Ok(exp_time) = chrono::DateTime::parse_from_rfc3339(expires) {
                return exp_time > chrono::Utc::now();
            }
        }
        true
    }

    /// Revoke this key.
    pub fn revoke(&mut self, reason: impl Into<String>) {
        self.revoked = true;
        self.revocation_reason = Some(reason.into());
    }
}

/// Storage for trusted public keys.
#[derive(Debug, Clone, Default)]
pub struct KeyStore {
    /// Keys indexed by key_id
    keys: HashMap<String, TrustedKey>,
    /// Keys indexed by fingerprint (for lookup by fingerprint)
    by_fingerprint: HashMap<String, String>,
}

impl KeyStore {
    /// Create an empty key store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a trusted key.
    pub fn add_key(&mut self, key: TrustedKey) -> TrustResult<()> {
        if let Some(fingerprint) = &key.fingerprint {
            self.by_fingerprint
                .insert(fingerprint.clone(), key.key_id.clone());
        }
        self.keys.insert(key.key_id.clone(), key);
        Ok(())
    }

    /// Remove a key by ID.
    pub fn remove_key(&mut self, key_id: &str) -> Option<TrustedKey> {
        if let Some(key) = self.keys.remove(key_id) {
            if let Some(fingerprint) = &key.fingerprint {
                self.by_fingerprint.remove(fingerprint);
            }
            Some(key)
        } else {
            None
        }
    }

    /// Get a key by ID.
    pub fn get_key(&self, key_id: &str) -> Option<&TrustedKey> {
        self.keys.get(key_id)
    }

    /// Get a key by fingerprint.
    pub fn get_key_by_fingerprint(&self, fingerprint: &str) -> Option<&TrustedKey> {
        self.by_fingerprint
            .get(fingerprint)
            .and_then(|id| self.keys.get(id))
    }

    /// List all keys.
    pub fn list_keys(&self) -> impl Iterator<Item = &TrustedKey> {
        self.keys.values()
    }

    /// List valid (non-expired, non-revoked) keys.
    pub fn list_valid_keys(&self) -> impl Iterator<Item = &TrustedKey> {
        self.keys.values().filter(|k| k.is_valid())
    }

    /// List keys with a minimum trust level.
    pub fn list_keys_with_trust(&self, min_trust: TrustLevel) -> impl Iterator<Item = &TrustedKey> {
        self.keys
            .values()
            .filter(move |k| k.is_valid() && k.trust_level >= min_trust)
    }

    /// Check if a key exists and is valid.
    pub fn has_valid_key(&self, key_id: &str) -> bool {
        self.keys.get(key_id).is_some_and(|k| k.is_valid())
    }

    /// Number of keys in the store.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Check if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Load from a directory (each key is a TOML file).
    pub fn load_from_directory(path: impl AsRef<Path>) -> TrustResult<Self> {
        let path = path.as_ref();
        let mut store = Self::new();

        if !path.exists() {
            return Ok(store);
        }

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_path = entry.path();
            if file_path.extension().is_some_and(|e| e == "toml") {
                let content = fs::read_to_string(&file_path)?;
                let key: TrustedKey = toml::from_str(&content)
                    .map_err(|e| TrustError::SerializationError(e.to_string()))?;
                store.add_key(key)?;
            }
        }

        Ok(store)
    }

    /// Save all keys to a directory.
    pub fn save_to_directory(&self, path: impl AsRef<Path>) -> TrustResult<()> {
        let path = path.as_ref();
        fs::create_dir_all(path)?;

        for key in self.keys.values() {
            let file_path = path.join(format!("{}.toml", key.key_id));
            let content = toml::to_string_pretty(key)
                .map_err(|e| TrustError::SerializationError(e.to_string()))?;
            fs::write(file_path, content)?;
        }

        Ok(())
    }

    /// Load from the default location (~/.dashflow/keys/).
    pub fn load_default() -> TrustResult<Self> {
        let path = dirs::home_dir()
            .map(|h| h.join(".dashflow").join("keys"))
            .unwrap_or_else(|| PathBuf::from(".dashflow/keys"));
        Self::load_from_directory(path)
    }

    /// Save to the default location.
    pub fn save_default(&self) -> TrustResult<()> {
        let path = dirs::home_dir()
            .map(|h| h.join(".dashflow").join("keys"))
            .unwrap_or_else(|| PathBuf::from(".dashflow/keys"));
        self.save_to_directory(path)
    }
}

/// Computes cryptographic hashes.
pub struct Hasher;

impl Hasher {
    /// Compute hash of bytes using the specified algorithm.
    pub fn hash_bytes(data: &[u8], algorithm: HashAlgorithm) -> String {
        match algorithm {
            HashAlgorithm::Sha256 => {
                let mut hasher = Sha256::new();
                hasher.update(data);
                hex::encode(hasher.finalize())
            }
            HashAlgorithm::Sha384 => {
                let mut hasher = Sha384::new();
                hasher.update(data);
                hex::encode(hasher.finalize())
            }
            HashAlgorithm::Sha512 => {
                let mut hasher = Sha512::new();
                hasher.update(data);
                hex::encode(hasher.finalize())
            }
            HashAlgorithm::Blake3 => {
                let hash = blake3::hash(data);
                hash.to_hex().to_string()
            }
        }
    }

    /// Compute hash of a file.
    pub fn hash_file(path: impl AsRef<Path>, algorithm: HashAlgorithm) -> TrustResult<String> {
        let data = fs::read(path)?;
        Ok(Self::hash_bytes(&data, algorithm))
    }

    /// Compute hash of a reader.
    pub fn hash_reader<R: Read>(mut reader: R, algorithm: HashAlgorithm) -> TrustResult<String> {
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer)?;
        Ok(Self::hash_bytes(&buffer, algorithm))
    }

    /// Verify that data matches an expected hash.
    pub fn verify_hash(data: &[u8], expected_hash: &str, algorithm: HashAlgorithm) -> bool {
        let actual = Self::hash_bytes(data, algorithm);
        // Constant-time comparison to prevent timing attacks
        actual.len() == expected_hash.len()
            && actual
                .bytes()
                .zip(expected_hash.bytes())
                .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                == 0
    }
}

/// Verifies package signatures.
pub struct PackageVerifier<'a> {
    key_store: &'a KeyStore,
}

impl<'a> PackageVerifier<'a> {
    /// Create a new verifier with the given key store.
    pub fn new(key_store: &'a KeyStore) -> Self {
        Self { key_store }
    }

    /// Verify a signature against content.
    pub fn verify_signature(
        &self,
        signature: &Signature,
        content: &[u8],
    ) -> TrustResult<VerificationResult> {
        // Get the key
        let key = self
            .key_store
            .get_key(&signature.key_id)
            .ok_or_else(|| TrustError::KeyNotFound(signature.key_id.clone()))?;

        // Check key validity
        if key.revoked {
            return Err(TrustError::KeyRevoked(signature.key_id.clone()));
        }
        if !key.is_valid() {
            return Err(TrustError::KeyExpired(signature.key_id.clone()));
        }

        // Check algorithm match
        if key.algorithm != signature.algorithm {
            return Err(TrustError::InvalidSignature(format!(
                "Algorithm mismatch: key uses {:?}, signature uses {:?}",
                key.algorithm, signature.algorithm
            )));
        }

        // Verify the content hash matches what was signed
        let content_hash = match &signature.signed_content {
            SignedContent::ManifestHash { hash, algorithm } => {
                let computed = Hasher::hash_bytes(content, *algorithm);
                if computed != *hash {
                    return Err(TrustError::HashMismatch {
                        expected: hash.clone(),
                        actual: computed,
                    });
                }
                hash.clone()
            }
            SignedContent::PackageHash { hash, algorithm } => {
                let computed = Hasher::hash_bytes(content, *algorithm);
                if computed != *hash {
                    return Err(TrustError::HashMismatch {
                        expected: hash.clone(),
                        actual: computed,
                    });
                }
                hash.clone()
            }
            SignedContent::Both {
                manifest_hash,
                package_hash: _,
                algorithm,
            } => {
                // For Both, we verify the manifest hash (caller should verify package separately)
                let computed = Hasher::hash_bytes(content, *algorithm);
                if computed != *manifest_hash {
                    return Err(TrustError::HashMismatch {
                        expected: manifest_hash.clone(),
                        actual: computed,
                    });
                }
                manifest_hash.clone()
            }
        };

        // Decode the signature
        let sig_bytes = BASE64
            .decode(&signature.signature)
            .map_err(|e| TrustError::InvalidSignature(format!("Invalid base64: {}", e)))?;

        // Verify the cryptographic signature
        let verified = match signature.algorithm {
            SignatureAlgorithm::Ed25519 => {
                self.verify_ed25519(&key.public_key_pem, &content_hash, &sig_bytes)?
            }
            SignatureAlgorithm::EcdsaP256 => {
                self.verify_ecdsa_p256(&key.public_key_pem, &content_hash, &sig_bytes)?
            }
            SignatureAlgorithm::RsaPss4096 => {
                self.verify_rsa_pss(&key.public_key_pem, &content_hash, &sig_bytes)?
            }
        };

        if verified {
            Ok(VerificationResult {
                valid: true,
                key_id: signature.key_id.clone(),
                trust_level: key.trust_level,
                timestamp: signature.timestamp.clone(),
            })
        } else {
            Err(TrustError::InvalidSignature(
                "Cryptographic verification failed".to_string(),
            ))
        }
    }

    /// Verify an Ed25519 signature.
    fn verify_ed25519(
        &self,
        public_key_pem: &str,
        message: &str,
        signature: &[u8],
    ) -> TrustResult<bool> {
        use ed25519_dalek::{Signature as Ed25519Sig, VerifyingKey};
        use pkcs8::DecodePublicKey;

        // Parse the public key from PEM
        let verifying_key = VerifyingKey::from_public_key_pem(public_key_pem)
            .map_err(|e| TrustError::KeyEncodingError(format!("Invalid Ed25519 key: {}", e)))?;

        // Parse the signature
        let sig = Ed25519Sig::try_from(signature)
            .map_err(|e| TrustError::InvalidSignature(format!("Invalid signature bytes: {}", e)))?;

        // Verify
        use ed25519_dalek::Verifier;
        Ok(verifying_key.verify(message.as_bytes(), &sig).is_ok())
    }

    /// Verify an ECDSA P-256 signature.
    fn verify_ecdsa_p256(
        &self,
        public_key_pem: &str,
        message: &str,
        signature: &[u8],
    ) -> TrustResult<bool> {
        use p256::ecdsa::{signature::Verifier, Signature as EcdsaSig, VerifyingKey};
        use p256::pkcs8::DecodePublicKey;

        // Parse the public key from PEM
        let verifying_key = VerifyingKey::from_public_key_pem(public_key_pem)
            .map_err(|e| TrustError::KeyEncodingError(format!("Invalid P-256 key: {}", e)))?;

        // Parse the signature (DER encoded)
        let sig = EcdsaSig::from_der(signature)
            .map_err(|e| TrustError::InvalidSignature(format!("Invalid signature bytes: {}", e)))?;

        // Verify
        Ok(verifying_key.verify(message.as_bytes(), &sig).is_ok())
    }

    /// Verify an RSA-PSS signature.
    fn verify_rsa_pss(
        &self,
        public_key_pem: &str,
        message: &str,
        signature: &[u8],
    ) -> TrustResult<bool> {
        use rsa::pkcs1v15::VerifyingKey as RsaVerifyingKey;
        use rsa::pkcs8::DecodePublicKey;
        use rsa::signature::Verifier;
        use rsa::RsaPublicKey;

        // Parse the public key from PEM
        let public_key = RsaPublicKey::from_public_key_pem(public_key_pem)
            .map_err(|e| TrustError::KeyEncodingError(format!("Invalid RSA key: {}", e)))?;

        // Create verifying key with SHA-256
        let verifying_key = RsaVerifyingKey::<Sha256>::new(public_key);

        // Parse the signature
        let sig = rsa::pkcs1v15::Signature::try_from(signature)
            .map_err(|e| TrustError::InvalidSignature(format!("Invalid signature bytes: {}", e)))?;

        // Verify
        Ok(verifying_key.verify(message.as_bytes(), &sig).is_ok())
    }

    /// Verify that content meets trust requirements.
    pub fn verify_trust(
        &self,
        signature: &Signature,
        content: &[u8],
        required_trust: TrustLevel,
    ) -> TrustResult<VerificationResult> {
        let result = self.verify_signature(signature, content)?;

        if result.trust_level < required_trust {
            return Err(TrustError::InsufficientTrust {
                required: required_trust,
                actual: result.trust_level,
            });
        }

        Ok(result)
    }
}

/// Result of signature verification.
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// Whether the signature is valid
    pub valid: bool,
    /// Key ID used for signing
    pub key_id: String,
    /// Trust level of the signing key
    pub trust_level: TrustLevel,
    /// Timestamp of the signature
    pub timestamp: String,
}

impl fmt::Display for VerificationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Signature {} (key: {}, trust: {:?}, time: {})",
            if self.valid { "VALID" } else { "INVALID" },
            self.key_id,
            self.trust_level,
            self.timestamp
        )
    }
}

/// Signs packages with a private key.
pub struct PackageSigner {
    /// Key identifier
    key_id: String,
    /// Private key in PEM format
    private_key_pem: String,
    /// Signing algorithm
    algorithm: SignatureAlgorithm,
}

impl PackageSigner {
    /// Create a new Ed25519 signer.
    pub fn new_ed25519(key_id: impl Into<String>, private_key_pem: impl Into<String>) -> Self {
        Self {
            key_id: key_id.into(),
            private_key_pem: private_key_pem.into(),
            algorithm: SignatureAlgorithm::Ed25519,
        }
    }

    /// Create a new ECDSA P-256 signer.
    pub fn new_ecdsa_p256(key_id: impl Into<String>, private_key_pem: impl Into<String>) -> Self {
        Self {
            key_id: key_id.into(),
            private_key_pem: private_key_pem.into(),
            algorithm: SignatureAlgorithm::EcdsaP256,
        }
    }

    /// Create a new RSA-PSS signer.
    pub fn new_rsa_pss(key_id: impl Into<String>, private_key_pem: impl Into<String>) -> Self {
        Self {
            key_id: key_id.into(),
            private_key_pem: private_key_pem.into(),
            algorithm: SignatureAlgorithm::RsaPss4096,
        }
    }

    /// Sign a manifest hash.
    pub fn sign_manifest(
        &self,
        manifest_content: &[u8],
        hash_algorithm: HashAlgorithm,
    ) -> TrustResult<Signature> {
        let hash = Hasher::hash_bytes(manifest_content, hash_algorithm);
        let sig_bytes = self.sign_message(&hash)?;

        Ok(Signature {
            key_id: self.key_id.clone(),
            algorithm: self.algorithm,
            signature: BASE64.encode(&sig_bytes),
            signed_content: SignedContent::ManifestHash {
                hash,
                algorithm: hash_algorithm,
            },
            timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Sign a package hash.
    pub fn sign_package(
        &self,
        package_content: &[u8],
        hash_algorithm: HashAlgorithm,
    ) -> TrustResult<Signature> {
        let hash = Hasher::hash_bytes(package_content, hash_algorithm);
        let sig_bytes = self.sign_message(&hash)?;

        Ok(Signature {
            key_id: self.key_id.clone(),
            algorithm: self.algorithm,
            signature: BASE64.encode(&sig_bytes),
            signed_content: SignedContent::PackageHash {
                hash,
                algorithm: hash_algorithm,
            },
            timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Sign both manifest and package.
    pub fn sign_both(
        &self,
        manifest_content: &[u8],
        package_content: &[u8],
        hash_algorithm: HashAlgorithm,
    ) -> TrustResult<Signature> {
        let manifest_hash = Hasher::hash_bytes(manifest_content, hash_algorithm);
        let package_hash = Hasher::hash_bytes(package_content, hash_algorithm);

        // Sign the manifest hash (canonical choice)
        let sig_bytes = self.sign_message(&manifest_hash)?;

        Ok(Signature {
            key_id: self.key_id.clone(),
            algorithm: self.algorithm,
            signature: BASE64.encode(&sig_bytes),
            signed_content: SignedContent::Both {
                manifest_hash,
                package_hash,
                algorithm: hash_algorithm,
            },
            timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Sign a raw message.
    fn sign_message(&self, message: &str) -> TrustResult<Vec<u8>> {
        match self.algorithm {
            SignatureAlgorithm::Ed25519 => self.sign_ed25519(message),
            SignatureAlgorithm::EcdsaP256 => self.sign_ecdsa_p256(message),
            SignatureAlgorithm::RsaPss4096 => self.sign_rsa_pss(message),
        }
    }

    /// Sign with Ed25519.
    fn sign_ed25519(&self, message: &str) -> TrustResult<Vec<u8>> {
        use ed25519_dalek::{Signer, SigningKey};
        use pkcs8::DecodePrivateKey;

        let signing_key = SigningKey::from_pkcs8_pem(&self.private_key_pem)
            .map_err(|e| TrustError::KeyEncodingError(format!("Invalid Ed25519 key: {}", e)))?;

        let signature = signing_key.sign(message.as_bytes());
        Ok(signature.to_bytes().to_vec())
    }

    /// Sign with ECDSA P-256.
    fn sign_ecdsa_p256(&self, message: &str) -> TrustResult<Vec<u8>> {
        use p256::ecdsa::{signature::Signer, SigningKey};
        use p256::pkcs8::DecodePrivateKey;

        let signing_key = SigningKey::from_pkcs8_pem(&self.private_key_pem)
            .map_err(|e| TrustError::KeyEncodingError(format!("Invalid P-256 key: {}", e)))?;

        let signature: p256::ecdsa::Signature = signing_key.sign(message.as_bytes());
        Ok(signature.to_der().to_bytes().to_vec())
    }

    /// Sign with RSA-PSS.
    fn sign_rsa_pss(&self, message: &str) -> TrustResult<Vec<u8>> {
        use rsa::pkcs1v15::SigningKey as RsaSigningKey;
        use rsa::pkcs8::DecodePrivateKey;
        use rsa::signature::{SignatureEncoding, Signer};
        use rsa::RsaPrivateKey;

        let private_key = RsaPrivateKey::from_pkcs8_pem(&self.private_key_pem)
            .map_err(|e| TrustError::KeyEncodingError(format!("Invalid RSA key: {}", e)))?;

        let signing_key = RsaSigningKey::<Sha256>::new(private_key);
        let signature = signing_key.sign(message.as_bytes());
        Ok(signature.to_vec())
    }

    /// Get the key ID.
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    /// Get the algorithm.
    pub fn algorithm(&self) -> SignatureAlgorithm {
        self.algorithm
    }
}

/// Generate a new Ed25519 key pair.
pub fn generate_ed25519_keypair() -> TrustResult<(String, String)> {
    use ed25519_dalek::SigningKey;
    use pkcs8::EncodePrivateKey;
    use pkcs8::EncodePublicKey;
    use rand_core::OsRng;

    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();

    let private_pem = signing_key
        .to_pkcs8_pem(pkcs8::LineEnding::LF)
        .map_err(|e| {
            TrustError::KeyEncodingError(format!("Failed to encode private key: {}", e))
        })?;

    let public_pem = verifying_key
        .to_public_key_pem(pkcs8::LineEnding::LF)
        .map_err(|e| TrustError::KeyEncodingError(format!("Failed to encode public key: {}", e)))?;

    Ok((private_pem.to_string(), public_pem))
}

/// Generate a new ECDSA P-256 key pair.
pub fn generate_ecdsa_p256_keypair() -> TrustResult<(String, String)> {
    use p256::ecdsa::SigningKey;
    use p256::pkcs8::EncodePrivateKey;
    use p256::pkcs8::EncodePublicKey;
    use rand_core::OsRng;

    let signing_key = SigningKey::random(&mut OsRng);
    let verifying_key = signing_key.verifying_key();

    let private_pem = signing_key
        .to_pkcs8_pem(p256::pkcs8::LineEnding::LF)
        .map_err(|e| {
            TrustError::KeyEncodingError(format!("Failed to encode private key: {}", e))
        })?;

    let public_pem = verifying_key
        .to_public_key_pem(p256::pkcs8::LineEnding::LF)
        .map_err(|e| TrustError::KeyEncodingError(format!("Failed to encode public key: {}", e)))?;

    Ok((private_pem.to_string(), public_pem))
}

/// Compute fingerprint (SHA-256) of a public key.
pub fn compute_key_fingerprint(public_key_pem: &str) -> String {
    let hash = Hasher::hash_bytes(public_key_pem.as_bytes(), HashAlgorithm::Sha256);
    // Format as colon-separated hex (like SSH fingerprints)
    hash.chars()
        .collect::<Vec<_>>()
        .chunks(2)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join(":")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hasher_sha256() {
        let data = b"hello world";
        let hash = Hasher::hash_bytes(data, HashAlgorithm::Sha256);
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_hasher_sha384() {
        let data = b"hello world";
        let hash = Hasher::hash_bytes(data, HashAlgorithm::Sha384);
        assert_eq!(hash.len(), 96); // 384 bits = 48 bytes = 96 hex chars
    }

    #[test]
    fn test_hasher_sha512() {
        let data = b"hello world";
        let hash = Hasher::hash_bytes(data, HashAlgorithm::Sha512);
        assert_eq!(hash.len(), 128); // 512 bits = 64 bytes = 128 hex chars
    }

    #[test]
    fn test_hasher_blake3() {
        let data = b"hello world";
        let hash = Hasher::hash_bytes(data, HashAlgorithm::Blake3);
        assert_eq!(hash.len(), 64); // BLAKE3 outputs 256 bits = 32 bytes = 64 hex chars
    }

    #[test]
    fn test_hasher_verify() {
        let data = b"test data";
        let hash = Hasher::hash_bytes(data, HashAlgorithm::Sha256);
        assert!(Hasher::verify_hash(data, &hash, HashAlgorithm::Sha256));
        assert!(!Hasher::verify_hash(
            b"wrong data",
            &hash,
            HashAlgorithm::Sha256
        ));
    }

    #[test]
    fn test_key_store_basic() {
        let mut store = KeyStore::new();
        assert!(store.is_empty());

        let key = TrustedKey::new_ed25519(
            "test-key",
            "-----BEGIN PUBLIC KEY-----\ntest\n-----END PUBLIC KEY-----",
            "Test User",
            TrustLevel::Community,
        );

        store.add_key(key).unwrap();
        assert_eq!(store.len(), 1);
        assert!(store.has_valid_key("test-key"));
        assert!(!store.has_valid_key("nonexistent"));

        let retrieved = store.get_key("test-key").unwrap();
        assert_eq!(retrieved.owner, "Test User");
        assert_eq!(retrieved.trust_level, TrustLevel::Community);
    }

    #[test]
    fn test_key_store_remove() {
        let mut store = KeyStore::new();
        let key = TrustedKey::new_ed25519(
            "to-remove",
            "-----BEGIN PUBLIC KEY-----\ntest\n-----END PUBLIC KEY-----",
            "Test",
            TrustLevel::Local,
        );

        store.add_key(key).unwrap();
        assert!(store.has_valid_key("to-remove"));

        let removed = store.remove_key("to-remove");
        assert!(removed.is_some());
        assert!(!store.has_valid_key("to-remove"));
    }

    #[test]
    fn test_key_revocation() {
        let mut key = TrustedKey::new_ed25519(
            "revoke-test",
            "-----BEGIN PUBLIC KEY-----\ntest\n-----END PUBLIC KEY-----",
            "Test",
            TrustLevel::Community,
        );

        assert!(key.is_valid());

        key.revoke("Security compromise");
        assert!(!key.is_valid());
        assert!(key.revoked);
        assert_eq!(
            key.revocation_reason,
            Some("Security compromise".to_string())
        );
    }

    #[test]
    fn test_trust_level_ordering() {
        assert!(TrustLevel::Local < TrustLevel::Community);
        assert!(TrustLevel::Community < TrustLevel::Verified);
        assert!(TrustLevel::Verified < TrustLevel::Official);
    }

    #[test]
    fn test_ed25519_sign_verify() {
        // Generate a key pair
        let (private_pem, public_pem) = generate_ed25519_keypair().unwrap();

        // Create signer
        let signer = PackageSigner::new_ed25519("test-key", &private_pem);

        // Sign some content
        let content = b"test package content";
        let signature = signer
            .sign_manifest(content, HashAlgorithm::Sha256)
            .unwrap();

        // Create key store and add public key
        let mut store = KeyStore::new();
        let trusted_key =
            TrustedKey::new_ed25519("test-key", &public_pem, "Test", TrustLevel::Local);
        store.add_key(trusted_key).unwrap();

        // Verify
        let verifier = PackageVerifier::new(&store);
        let result = verifier.verify_signature(&signature, content).unwrap();
        assert!(result.valid);
        assert_eq!(result.key_id, "test-key");
        assert_eq!(result.trust_level, TrustLevel::Local);
    }

    #[test]
    fn test_ecdsa_p256_sign_verify() {
        // Generate a key pair
        let (private_pem, public_pem) = generate_ecdsa_p256_keypair().unwrap();

        // Create signer
        let signer = PackageSigner::new_ecdsa_p256("ecdsa-key", &private_pem);

        // Sign some content
        let content = b"ecdsa test content";
        let signature = signer.sign_package(content, HashAlgorithm::Blake3).unwrap();

        // Create key store and add public key
        let mut store = KeyStore::new();
        let trusted_key = TrustedKey::new_ecdsa_p256(
            "ecdsa-key",
            &public_pem,
            "ECDSA User",
            TrustLevel::Verified,
        );
        store.add_key(trusted_key).unwrap();

        // Verify
        let verifier = PackageVerifier::new(&store);
        let result = verifier.verify_signature(&signature, content).unwrap();
        assert!(result.valid);
        assert_eq!(result.trust_level, TrustLevel::Verified);
    }

    #[test]
    fn test_sign_both() {
        let (private_pem, public_pem) = generate_ed25519_keypair().unwrap();

        let signer = PackageSigner::new_ed25519("both-key", &private_pem);

        let manifest = b"manifest content";
        let package = b"package content";
        let signature = signer
            .sign_both(manifest, package, HashAlgorithm::Sha256)
            .unwrap();

        // Verify signed_content structure
        match &signature.signed_content {
            SignedContent::Both {
                manifest_hash,
                package_hash,
                algorithm,
            } => {
                assert_eq!(*algorithm, HashAlgorithm::Sha256);
                assert_eq!(
                    manifest_hash,
                    &Hasher::hash_bytes(manifest, HashAlgorithm::Sha256)
                );
                assert_eq!(
                    package_hash,
                    &Hasher::hash_bytes(package, HashAlgorithm::Sha256)
                );
            }
            _ => panic!("Expected Both signed content"),
        }

        // Verify the signature
        let mut store = KeyStore::new();
        store
            .add_key(TrustedKey::new_ed25519(
                "both-key",
                &public_pem,
                "Test",
                TrustLevel::Official,
            ))
            .unwrap();

        let verifier = PackageVerifier::new(&store);
        let result = verifier.verify_signature(&signature, manifest).unwrap();
        assert!(result.valid);
    }

    #[test]
    fn test_hash_mismatch_error() {
        let (private_pem, public_pem) = generate_ed25519_keypair().unwrap();

        let signer = PackageSigner::new_ed25519("mismatch-key", &private_pem);
        let content = b"original content";
        let signature = signer
            .sign_manifest(content, HashAlgorithm::Sha256)
            .unwrap();

        let mut store = KeyStore::new();
        store
            .add_key(TrustedKey::new_ed25519(
                "mismatch-key",
                &public_pem,
                "Test",
                TrustLevel::Local,
            ))
            .unwrap();

        let verifier = PackageVerifier::new(&store);

        // Verify with different content should fail
        let result = verifier.verify_signature(&signature, b"different content");
        assert!(matches!(result, Err(TrustError::HashMismatch { .. })));
    }

    #[test]
    fn test_key_not_found_error() {
        let store = KeyStore::new();
        let verifier = PackageVerifier::new(&store);

        let signature = Signature::new(
            "nonexistent-key",
            SignatureAlgorithm::Ed25519,
            "dummy",
            SignedContent::ManifestHash {
                hash: "abc".to_string(),
                algorithm: HashAlgorithm::Sha256,
            },
            "2025-01-01T00:00:00Z",
        );

        let result = verifier.verify_signature(&signature, b"content");
        assert!(matches!(result, Err(TrustError::KeyNotFound(_))));
    }

    #[test]
    fn test_insufficient_trust_error() {
        let (private_pem, public_pem) = generate_ed25519_keypair().unwrap();

        let signer = PackageSigner::new_ed25519("low-trust-key", &private_pem);
        let content = b"content";
        let signature = signer
            .sign_manifest(content, HashAlgorithm::Sha256)
            .unwrap();

        let mut store = KeyStore::new();
        store
            .add_key(TrustedKey::new_ed25519(
                "low-trust-key",
                &public_pem,
                "Test",
                TrustLevel::Local,
            ))
            .unwrap();

        let verifier = PackageVerifier::new(&store);

        // Should fail when requiring higher trust
        let result = verifier.verify_trust(&signature, content, TrustLevel::Official);
        assert!(matches!(result, Err(TrustError::InsufficientTrust { .. })));

        // Should succeed when requiring same or lower trust
        let result = verifier.verify_trust(&signature, content, TrustLevel::Local);
        assert!(result.is_ok());
    }

    #[test]
    fn test_key_fingerprint() {
        let fingerprint =
            compute_key_fingerprint("-----BEGIN PUBLIC KEY-----\ntest\n-----END PUBLIC KEY-----");
        // Should be colon-separated hex
        assert!(fingerprint.contains(':'));
        assert_eq!(fingerprint.chars().filter(|c| *c == ':').count(), 31); // 64 hex chars / 2 - 1 colons
    }

    #[test]
    fn test_list_keys_with_trust() {
        let mut store = KeyStore::new();

        store
            .add_key(TrustedKey::new_ed25519(
                "local-key",
                "pem1",
                "User1",
                TrustLevel::Local,
            ))
            .unwrap();
        store
            .add_key(TrustedKey::new_ed25519(
                "community-key",
                "pem2",
                "User2",
                TrustLevel::Community,
            ))
            .unwrap();
        store
            .add_key(TrustedKey::new_ed25519(
                "verified-key",
                "pem3",
                "User3",
                TrustLevel::Verified,
            ))
            .unwrap();
        store
            .add_key(TrustedKey::new_ed25519(
                "official-key",
                "pem4",
                "User4",
                TrustLevel::Official,
            ))
            .unwrap();

        let verified_plus: Vec<_> = store.list_keys_with_trust(TrustLevel::Verified).collect();
        assert_eq!(verified_plus.len(), 2);

        let official_only: Vec<_> = store.list_keys_with_trust(TrustLevel::Official).collect();
        assert_eq!(official_only.len(), 1);

        let all: Vec<_> = store.list_keys_with_trust(TrustLevel::Local).collect();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn test_verification_result_display() {
        let result = VerificationResult {
            valid: true,
            key_id: "test-key".to_string(),
            trust_level: TrustLevel::Verified,
            timestamp: "2025-01-01T00:00:00Z".to_string(),
        };

        let display = format!("{}", result);
        assert!(display.contains("VALID"));
        assert!(display.contains("test-key"));
        assert!(display.contains("Verified"));
    }
}
