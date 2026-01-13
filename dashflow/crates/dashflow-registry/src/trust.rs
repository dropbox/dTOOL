//! Trust service for package verification.
//!
//! Provides cryptographic verification of packages and lineage chains.
//! Every package operation should verify signatures before trusting content.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_registry::{TrustService, Keyring, SignedPackage};
//!
//! let keyring = Keyring::new();
//! let trust_service = TrustService::new(keyring);
//!
//! let result = trust_service.verify_package(&signed_package)?;
//! if result.verified {
//!     println!("Package verified at trust level: {:?}", result.trust_level);
//! }
//! ```

use std::collections::HashMap;

use crate::content_hash::ContentHash;
use crate::error::{RegistryError, Result};
use crate::package::{Lineage, LineageStep, TrustLevel};
use crate::signature::{PublicKey, SignedPackage};
use chrono::{DateTime, Utc};

/// A collection of trusted public keys.
///
/// The keyring stores public keys organized by trust level and allows
/// lookup by key ID for signature verification.
#[derive(Debug, Clone, Default)]
pub struct Keyring {
    /// Keys indexed by key_id.
    keys: HashMap<String, KeyEntry>,
}

/// An entry in the keyring with associated metadata.
#[derive(Debug, Clone)]
pub struct KeyEntry {
    /// The public key.
    pub key: PublicKey,
    /// Trust level assigned to this key.
    pub trust_level: TrustLevel,
    /// When this key was added to the keyring.
    pub added_at: DateTime<Utc>,
    /// Optional expiration time.
    pub expires_at: Option<DateTime<Utc>>,
    /// Is this key revoked?
    pub revoked: bool,
}

impl Keyring {
    /// Create a new empty keyring.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a key with a specific trust level.
    pub fn add_key(&mut self, key: &PublicKey, trust_level: TrustLevel) {
        let entry = KeyEntry {
            key: key.clone(),
            trust_level,
            added_at: Utc::now(),
            expires_at: None,
            revoked: false,
        };
        self.keys.insert(key.key_id.clone(), entry);
    }

    /// Add a key with expiration.
    pub fn add_key_with_expiry(
        &mut self,
        key: &PublicKey,
        trust_level: TrustLevel,
        expires_at: DateTime<Utc>,
    ) {
        let entry = KeyEntry {
            key: key.clone(),
            trust_level,
            added_at: Utc::now(),
            expires_at: Some(expires_at),
            revoked: false,
        };
        self.keys.insert(key.key_id.clone(), entry);
    }

    /// Get a key by ID.
    pub fn get(&self, key_id: &str) -> Option<&KeyEntry> {
        self.keys.get(key_id).filter(|entry| self.is_valid(entry))
    }

    /// Get a key's public key by ID.
    pub fn get_key(&self, key_id: &str) -> Option<&PublicKey> {
        self.get(key_id).map(|e| &e.key)
    }

    /// Check if a key entry is currently valid.
    fn is_valid(&self, entry: &KeyEntry) -> bool {
        if entry.revoked {
            return false;
        }
        if let Some(expires_at) = entry.expires_at {
            if Utc::now() > expires_at {
                return false;
            }
        }
        true
    }

    /// Revoke a key by ID.
    pub fn revoke(&mut self, key_id: &str) -> bool {
        if let Some(entry) = self.keys.get_mut(key_id) {
            entry.revoked = true;
            true
        } else {
            false
        }
    }

    /// List all active keys.
    pub fn active_keys(&self) -> Vec<&KeyEntry> {
        self.keys.values().filter(|e| self.is_valid(e)).collect()
    }

    /// List keys by trust level.
    pub fn keys_by_trust_level(&self, level: TrustLevel) -> Vec<&KeyEntry> {
        self.active_keys()
            .into_iter()
            .filter(|e| e.trust_level >= level)
            .collect()
    }

    /// Check if the keyring contains a valid key with the given ID.
    pub fn contains(&self, key_id: &str) -> bool {
        self.get(key_id).is_some()
    }

    /// Number of active keys.
    pub fn len(&self) -> usize {
        self.active_keys().len()
    }

    /// Is the keyring empty?
    pub fn is_empty(&self) -> bool {
        self.active_keys().is_empty()
    }
}

/// Result of verifying a single signature.
#[derive(Debug, Clone)]
pub struct SignatureVerification {
    /// Key ID used for this signature.
    pub key_id: String,
    /// Owner of the signing key.
    pub key_owner: String,
    /// Trust level of the signing key.
    pub trust_level: TrustLevel,
    /// Was the signature valid?
    pub valid: bool,
    /// When the signature was created.
    pub timestamp: DateTime<Utc>,
}

/// Result of verifying a package.
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// Results for each signature on the package.
    pub signatures: Vec<SignatureVerification>,
    /// Overall trust level (highest valid signature).
    pub trust_level: TrustLevel,
    /// Is the package verified (at least one valid signature)?
    pub verified: bool,
    /// Number of valid signatures.
    pub valid_signature_count: usize,
    /// Number of invalid signatures.
    pub invalid_signature_count: usize,
}

/// Result of verifying a single lineage step.
#[derive(Debug, Clone)]
pub struct LineageStepVerification {
    /// The step that was verified.
    pub step_id: uuid::Uuid,
    /// Was this step's signature valid?
    pub valid: bool,
    /// Error message if invalid.
    pub error: Option<String>,
}

/// Result of verifying a lineage chain.
#[derive(Debug, Clone)]
pub struct LineageVerification {
    /// Is the entire chain valid?
    pub chain_valid: bool,
    /// Verification results for each step.
    pub steps: Vec<LineageStepVerification>,
    /// The original package this was derived from.
    pub original: Option<ContentHash>,
    /// Total number of steps in the chain.
    pub chain_length: usize,
}

/// Trust verification service.
///
/// Verifies package signatures and lineage chains using a keyring
/// of trusted public keys.
pub struct TrustService {
    /// Known trusted keys.
    keyring: Keyring,
    /// Minimum trust level required for verification.
    min_trust_level: TrustLevel,
}

impl TrustService {
    /// Create a new trust service with a keyring.
    pub fn new(keyring: Keyring) -> Self {
        Self {
            keyring,
            min_trust_level: TrustLevel::Community,
        }
    }

    /// Create a trust service with a minimum trust level requirement.
    pub fn with_min_trust_level(keyring: Keyring, min_trust_level: TrustLevel) -> Self {
        Self {
            keyring,
            min_trust_level,
        }
    }

    /// Get a reference to the keyring.
    pub fn keyring(&self) -> &Keyring {
        &self.keyring
    }

    /// Get a mutable reference to the keyring.
    pub fn keyring_mut(&mut self) -> &mut Keyring {
        &mut self.keyring
    }

    /// Verify a signed package.
    ///
    /// Checks all signatures against the keyring and determines
    /// the overall trust level.
    pub fn verify_package(&self, package: &SignedPackage) -> Result<VerificationResult> {
        let mut signatures = Vec::new();
        let mut max_trust_level = TrustLevel::Unknown;
        let mut valid_count = 0;
        let mut invalid_count = 0;

        for sig in &package.signatures {
            // Look up the key
            let key_entry = match self.keyring.get(&sig.key_id) {
                Some(entry) => entry,
                None => {
                    // Unknown key - signature cannot be verified
                    signatures.push(SignatureVerification {
                        key_id: sig.key_id.clone(),
                        key_owner: "unknown".to_string(),
                        trust_level: TrustLevel::Unknown,
                        valid: false,
                        timestamp: sig.timestamp,
                    });
                    invalid_count += 1;
                    continue;
                }
            };

            // Verify the signature
            let valid = sig
                .verify_package(&package.manifest, &package.hash, &key_entry.key)
                .unwrap_or(false);

            if valid {
                valid_count += 1;
                if key_entry.trust_level > max_trust_level {
                    max_trust_level = key_entry.trust_level;
                }
            } else {
                invalid_count += 1;
            }

            signatures.push(SignatureVerification {
                key_id: sig.key_id.clone(),
                key_owner: key_entry.key.owner.clone(),
                trust_level: key_entry.trust_level,
                valid,
                timestamp: sig.timestamp,
            });
        }

        // Package is verified if at least one valid signature meets minimum trust
        let verified = valid_count > 0 && max_trust_level >= self.min_trust_level;

        Ok(VerificationResult {
            signatures,
            trust_level: max_trust_level,
            verified,
            valid_signature_count: valid_count,
            invalid_signature_count: invalid_count,
        })
    }

    /// Verify a lineage chain.
    ///
    /// Each step in the lineage must have a valid signature from a trusted key.
    pub fn verify_lineage(&self, lineage: &Lineage) -> Result<LineageVerification> {
        let mut step_results = Vec::new();
        let mut chain_valid = true;

        for step in &lineage.chain {
            let step_valid = self.verify_lineage_step(step)?;

            if !step_valid.valid {
                chain_valid = false;
            }

            step_results.push(step_valid);
        }

        Ok(LineageVerification {
            chain_valid,
            steps: step_results,
            original: lineage.derived_from.clone(),
            chain_length: lineage.chain.len(),
        })
    }

    /// Verify a single lineage step.
    fn verify_lineage_step(&self, step: &LineageStep) -> Result<LineageStepVerification> {
        // The signature field contains key_id:signature_hex
        let parts: Vec<&str> = step.signature.split(':').collect();

        if parts.len() != 2 {
            return Ok(LineageStepVerification {
                step_id: step.id,
                valid: false,
                error: Some("Invalid signature format".to_string()),
            });
        }

        let key_id = parts[0];
        let sig_hex = parts[1];

        // Look up the key
        let key_entry = match self.keyring.get(key_id) {
            Some(entry) => entry,
            None => {
                return Ok(LineageStepVerification {
                    step_id: step.id,
                    valid: false,
                    error: Some(format!("Unknown key: {}", key_id)),
                });
            }
        };

        // Decode the signature
        let sig_bytes = match hex::decode(sig_hex) {
            Ok(bytes) => bytes,
            Err(e) => {
                return Ok(LineageStepVerification {
                    step_id: step.id,
                    valid: false,
                    error: Some(format!("Invalid signature hex: {}", e)),
                });
            }
        };

        // Create the content to verify (step details)
        let signed_content = create_lineage_step_content(step);

        // Verify
        let valid = verify_raw_signature(&signed_content, &sig_bytes, &key_entry.key)?;

        Ok(LineageStepVerification {
            step_id: step.id,
            valid,
            error: if valid {
                None
            } else {
                Some("Signature verification failed".to_string())
            },
        })
    }

    /// Calculate the trust level for a package based on its signatures.
    pub fn calculate_trust_level(&self, package: &SignedPackage) -> TrustLevel {
        let mut max_level = TrustLevel::Unknown;

        for sig in &package.signatures {
            if let Some(entry) = self.keyring.get(&sig.key_id) {
                let valid = sig
                    .verify_package(&package.manifest, &package.hash, &entry.key)
                    .unwrap_or(false);

                if valid && entry.trust_level > max_level {
                    max_level = entry.trust_level;
                }
            }
        }

        max_level
    }

    /// Verify a signature on raw data against a public key.
    ///
    /// This is useful for verifying signatures on API requests.
    pub fn verify_data_signature(
        &self,
        data: &[u8],
        signature: &crate::Signature,
        public_key: &crate::PublicKey,
    ) -> Result<bool> {
        verify_raw_signature(data, &signature.signature, public_key)
    }

    /// List all keys in the keyring.
    pub fn list_keys(&self) -> Vec<&KeyEntry> {
        self.keyring.active_keys()
    }

    /// Get a key by ID.
    pub fn get_key(&self, key_id: &str) -> Option<&KeyEntry> {
        self.keyring.get(key_id)
    }
}

/// Create the canonical content for signing a lineage step.
fn create_lineage_step_content(step: &LineageStep) -> Vec<u8> {
    format!(
        "dashflow-lineage:{}:{}:{}:{:?}:{}:{}",
        step.id,
        step.source_hash,
        step.result_hash,
        step.derivation_type,
        step.actor,
        step.timestamp.timestamp()
    )
    .into_bytes()
}

/// Verify a raw Ed25519 signature.
fn verify_raw_signature(data: &[u8], signature: &[u8], public_key: &PublicKey) -> Result<bool> {
    use ed25519_dalek::Verifier;

    let verifying_key = public_key.verifying_key()?;

    let sig_bytes: [u8; 64] = signature.try_into().map_err(|_| {
        RegistryError::InvalidSignature(format!(
            "signature must be 64 bytes, got {}",
            signature.len()
        ))
    })?;

    let sig = ed25519_dalek::Signature::from_bytes(&sig_bytes);

    Ok(verifying_key.verify(data, &sig).is_ok())
}

/// Builder for creating signed lineage steps.
pub struct LineageStepBuilder {
    source_hash: Option<ContentHash>,
    result_hash: Option<ContentHash>,
    derivation_type: Option<crate::package::DerivationType>,
    actor: Option<String>,
}

impl LineageStepBuilder {
    /// Create a new lineage step builder.
    pub fn new() -> Self {
        Self {
            source_hash: None,
            result_hash: None,
            derivation_type: None,
            actor: None,
        }
    }

    /// Set the source package hash.
    pub fn source(mut self, hash: ContentHash) -> Self {
        self.source_hash = Some(hash);
        self
    }

    /// Set the result package hash.
    pub fn result(mut self, hash: ContentHash) -> Self {
        self.result_hash = Some(hash);
        self
    }

    /// Set the derivation type.
    pub fn derivation_type(mut self, dt: crate::package::DerivationType) -> Self {
        self.derivation_type = Some(dt);
        self
    }

    /// Set the actor (who performed this derivation).
    pub fn actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }

    /// Build and sign the lineage step.
    pub fn build_and_sign(self, keypair: &crate::signature::KeyPair) -> Result<LineageStep> {
        let source_hash = self
            .source_hash
            .ok_or_else(|| RegistryError::InvalidInput("source_hash is required".into()))?;
        let result_hash = self
            .result_hash
            .ok_or_else(|| RegistryError::InvalidInput("result_hash is required".into()))?;
        let derivation_type = self
            .derivation_type
            .ok_or_else(|| RegistryError::InvalidInput("derivation_type is required".into()))?;
        let actor = self
            .actor
            .ok_or_else(|| RegistryError::InvalidInput("actor is required".into()))?;

        let id = uuid::Uuid::new_v4();
        let timestamp = Utc::now();

        // Create the step without signature first
        let mut step = LineageStep {
            id,
            source_hash,
            result_hash,
            derivation_type,
            actor,
            timestamp,
            signature: String::new(),
        };

        // Sign it
        let content = create_lineage_step_content(&step);
        let sig = keypair.sign(&content);

        // Format signature as key_id:hex_signature
        step.signature = format!("{}:{}", sig.key_id, hex::encode(&sig.signature));

        Ok(step)
    }
}

impl Default for LineageStepBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::PackageManifest;
    use crate::signature::KeyPair;

    fn test_manifest() -> PackageManifest {
        PackageManifest::builder()
            .name("test-package")
            .version("1.0.0")
            .description("Test")
            .build()
            .unwrap()
    }

    #[test]
    fn test_keyring_add_and_get() {
        let mut keyring = Keyring::new();
        let keypair = KeyPair::generate("test-owner".to_string());

        keyring.add_key(&keypair.public_key, TrustLevel::Community);

        assert!(keyring.contains(&keypair.public_key.key_id));
        assert_eq!(keyring.len(), 1);

        let entry = keyring.get(&keypair.public_key.key_id).unwrap();
        assert_eq!(entry.trust_level, TrustLevel::Community);
    }

    #[test]
    fn test_keyring_revoke() {
        let mut keyring = Keyring::new();
        let keypair = KeyPair::generate("test".to_string());

        keyring.add_key(&keypair.public_key, TrustLevel::Community);
        assert!(keyring.contains(&keypair.public_key.key_id));

        keyring.revoke(&keypair.public_key.key_id);
        assert!(!keyring.contains(&keypair.public_key.key_id));
    }

    #[test]
    fn test_keyring_expiry() {
        let mut keyring = Keyring::new();
        let keypair = KeyPair::generate("test".to_string());

        // Add key that expired in the past
        keyring.add_key_with_expiry(
            &keypair.public_key,
            TrustLevel::Community,
            Utc::now() - chrono::Duration::hours(1),
        );

        // Should not be accessible
        assert!(!keyring.contains(&keypair.public_key.key_id));
    }

    #[test]
    fn test_trust_service_verify_package() {
        let mut keyring = Keyring::new();
        let keypair = KeyPair::generate("publisher".to_string());

        keyring.add_key(&keypair.public_key, TrustLevel::Organization);

        let trust_service = TrustService::new(keyring);

        let manifest = test_manifest();
        let hash = ContentHash::from_bytes(b"package-content");
        let mut signed = SignedPackage::new(manifest.clone(), hash.clone());

        let signature = keypair.sign_package(&manifest, &hash);
        signed.add_signature(signature);

        let result = trust_service.verify_package(&signed).unwrap();

        assert!(result.verified);
        assert_eq!(result.trust_level, TrustLevel::Organization);
        assert_eq!(result.valid_signature_count, 1);
        assert_eq!(result.invalid_signature_count, 0);
    }

    #[test]
    fn test_trust_service_unknown_key() {
        let keyring = Keyring::new(); // Empty keyring
        let trust_service = TrustService::new(keyring);

        let keypair = KeyPair::generate("unknown".to_string());
        let manifest = test_manifest();
        let hash = ContentHash::from_bytes(b"content");
        let mut signed = SignedPackage::new(manifest.clone(), hash.clone());

        let signature = keypair.sign_package(&manifest, &hash);
        signed.add_signature(signature);

        let result = trust_service.verify_package(&signed).unwrap();

        assert!(!result.verified);
        assert_eq!(result.trust_level, TrustLevel::Unknown);
        assert_eq!(result.valid_signature_count, 0);
        assert_eq!(result.invalid_signature_count, 1);
    }

    #[test]
    fn test_trust_service_min_trust_level() {
        let mut keyring = Keyring::new();
        let keypair = KeyPair::generate("community".to_string());

        // Add as Community level
        keyring.add_key(&keypair.public_key, TrustLevel::Community);

        // Require Organization level
        let trust_service = TrustService::with_min_trust_level(keyring, TrustLevel::Organization);

        let manifest = test_manifest();
        let hash = ContentHash::from_bytes(b"content");
        let mut signed = SignedPackage::new(manifest.clone(), hash.clone());

        let signature = keypair.sign_package(&manifest, &hash);
        signed.add_signature(signature);

        let result = trust_service.verify_package(&signed).unwrap();

        // Valid signature but doesn't meet trust level
        assert!(!result.verified);
        assert_eq!(result.trust_level, TrustLevel::Community);
        assert_eq!(result.valid_signature_count, 1);
    }

    #[test]
    fn test_calculate_trust_level() {
        let mut keyring = Keyring::new();

        let community_key = KeyPair::generate("community".to_string());
        let official_key = KeyPair::generate("official".to_string());

        keyring.add_key(&community_key.public_key, TrustLevel::Community);
        keyring.add_key(&official_key.public_key, TrustLevel::Official);

        let trust_service = TrustService::new(keyring);

        let manifest = test_manifest();
        let hash = ContentHash::from_bytes(b"content");
        let mut signed = SignedPackage::new(manifest.clone(), hash.clone());

        // Sign with both keys
        signed.add_signature(community_key.sign_package(&manifest, &hash));
        signed.add_signature(official_key.sign_package(&manifest, &hash));

        // Should return highest trust level
        let level = trust_service.calculate_trust_level(&signed);
        assert_eq!(level, TrustLevel::Official);
    }

    #[test]
    fn test_lineage_step_builder() {
        let keypair = KeyPair::generate("deriver".to_string());

        let source = ContentHash::from_bytes(b"source");
        let result = ContentHash::from_bytes(b"result");

        let step = LineageStepBuilder::new()
            .source(source.clone())
            .result(result.clone())
            .derivation_type(crate::package::DerivationType::BugFix)
            .actor("test-actor")
            .build_and_sign(&keypair)
            .unwrap();

        assert_eq!(step.source_hash, source);
        assert_eq!(step.result_hash, result);
        assert_eq!(step.derivation_type, crate::package::DerivationType::BugFix);
        assert!(!step.signature.is_empty());
        assert!(step.signature.starts_with(&keypair.public_key.key_id));
    }

    #[test]
    fn test_verify_lineage() {
        let mut keyring = Keyring::new();
        let keypair = KeyPair::generate("deriver".to_string());

        keyring.add_key(&keypair.public_key, TrustLevel::Community);

        let trust_service = TrustService::new(keyring);

        let source = ContentHash::from_bytes(b"original");
        let result = ContentHash::from_bytes(b"derived");

        let step = LineageStepBuilder::new()
            .source(source.clone())
            .result(result.clone())
            .derivation_type(crate::package::DerivationType::Enhancement)
            .actor("test-deriver")
            .build_and_sign(&keypair)
            .unwrap();

        let lineage = Lineage {
            derived_from: Some(source),
            chain: vec![step],
        };

        let result = trust_service.verify_lineage(&lineage).unwrap();

        assert!(result.chain_valid);
        assert_eq!(result.chain_length, 1);
        assert!(result.steps[0].valid);
    }

    #[test]
    fn test_keys_by_trust_level() {
        let mut keyring = Keyring::new();

        let k1 = KeyPair::generate("community1".to_string());
        let k2 = KeyPair::generate("org1".to_string());
        let k3 = KeyPair::generate("official1".to_string());

        keyring.add_key(&k1.public_key, TrustLevel::Community);
        keyring.add_key(&k2.public_key, TrustLevel::Organization);
        keyring.add_key(&k3.public_key, TrustLevel::Official);

        let org_keys = keyring.keys_by_trust_level(TrustLevel::Organization);
        assert_eq!(org_keys.len(), 2); // Org and Official

        let official_keys = keyring.keys_by_trust_level(TrustLevel::Official);
        assert_eq!(official_keys.len(), 1); // Only Official
    }
}
