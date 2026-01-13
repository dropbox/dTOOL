// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Checkpoint encryption support using ChaCha20-Poly1305
//!
//! Provides application-level encryption for checkpoint state data.
//!
//! # Features
//!
//! - ChaCha20-Poly1305 AEAD encryption (256-bit key, 96-bit nonce)
//! - Argon2id key derivation from passphrases
//! - Random nonce generation per encryption
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::checkpoint::encryption::{EncryptionKey, encrypt_bytes, decrypt_bytes};
//!
//! // Create key from passphrase
//! let key = EncryptionKey::from_passphrase("my-secure-password")?;
//!
//! // Encrypt data
//! let encrypted = encrypt_bytes(&key, &plaintext)?;
//!
//! // Decrypt data
//! let decrypted = decrypt_bytes(&key, &encrypted)?;
//! ```

use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce,
};

use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};

use crate::error::{CheckpointError, Error, Result};

/// Encryption key for checkpoint data (256-bit)
#[derive(Clone)]
pub struct EncryptionKey {
    key: [u8; 32],
}

impl EncryptionKey {
    /// Create encryption key from raw 32-byte key
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 32 {
            return Err(Error::Checkpoint(CheckpointError::EncryptionFailed {
                reason: format!("Key must be exactly 32 bytes, got {}", bytes.len()),
            }));
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(bytes);
        Ok(Self { key })
    }

    /// Derive encryption key from passphrase using Argon2id
    pub fn from_passphrase(passphrase: &str) -> Result<Self> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(passphrase.as_bytes(), &salt)
            .map_err(|e| {
                Error::Checkpoint(CheckpointError::EncryptionFailed {
                    reason: format!("Key derivation failed: {}", e),
                })
            })?;

        let hash_output = password_hash.hash.ok_or_else(|| {
            Error::Checkpoint(CheckpointError::EncryptionFailed {
                reason: "Argon2 did not produce hash output".to_string(),
            })
        })?;

        let hash_bytes = hash_output.as_bytes();
        if hash_bytes.len() < 32 {
            return Err(Error::Checkpoint(CheckpointError::EncryptionFailed {
                reason: format!("Hash too short: {} bytes", hash_bytes.len()),
            }));
        }

        let mut key = [0u8; 32];
        key.copy_from_slice(&hash_bytes[..32]);
        Ok(Self { key })
    }

    /// Get the raw key bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.key
    }
}

impl std::fmt::Debug for EncryptionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncryptionKey")
            .field("key", &"[REDACTED]")
            .finish()
    }
}

impl Drop for EncryptionKey {
    fn drop(&mut self) {
        self.key.fill(0); // Zero out key on drop
    }
}

/// Encrypt bytes using ChaCha20-Poly1305
///
/// Returns: nonce (12 bytes) + ciphertext + auth tag (16 bytes)
pub fn encrypt_bytes(key: &EncryptionKey, plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = ChaCha20Poly1305::new(key.as_bytes().into());
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);

    let ciphertext = cipher.encrypt(&nonce, plaintext).map_err(|e| {
        Error::Checkpoint(CheckpointError::EncryptionFailed {
            reason: format!("Encryption failed: {}", e),
        })
    })?;

    let mut result = Vec::with_capacity(12 + ciphertext.len());
    result.extend_from_slice(&nonce);
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

/// Decrypt bytes using ChaCha20-Poly1305
///
/// Expects: nonce (12 bytes) + ciphertext + auth tag (16 bytes)
pub fn decrypt_bytes(key: &EncryptionKey, ciphertext: &[u8]) -> Result<Vec<u8>> {
    if ciphertext.len() < 28 {
        return Err(Error::Checkpoint(CheckpointError::DecryptionFailed {
            reason: format!("Ciphertext too short: {} bytes", ciphertext.len()),
        }));
    }

    let cipher = ChaCha20Poly1305::new(key.as_bytes().into());
    let nonce = Nonce::from_slice(&ciphertext[..12]);

    cipher.decrypt(nonce, &ciphertext[12..]).map_err(|e| {
        Error::Checkpoint(CheckpointError::DecryptionFailed {
            reason: format!("Decryption failed (wrong key or corrupted): {}", e),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_key_from_bytes() {
        let key = EncryptionKey::from_bytes(&[42u8; 32]).unwrap();
        assert_eq!(key.as_bytes().len(), 32);
    }

    #[test]
    fn test_encryption_key_wrong_size() {
        assert!(EncryptionKey::from_bytes(&[0u8; 16]).is_err());
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = EncryptionKey::from_bytes(&[42u8; 32]).unwrap();
        let plaintext = b"Hello, encrypted checkpoints!";

        let encrypted = encrypt_bytes(&key, plaintext).unwrap();
        assert_eq!(encrypted.len(), plaintext.len() + 28);

        let decrypted = decrypt_bytes(&key, &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_wrong_key_fails() {
        let key1 = EncryptionKey::from_bytes(&[1u8; 32]).unwrap();
        let key2 = EncryptionKey::from_bytes(&[2u8; 32]).unwrap();

        let encrypted = encrypt_bytes(&key1, b"secret").unwrap();
        assert!(decrypt_bytes(&key2, &encrypted).is_err());
    }

    #[test]
    fn test_corrupted_data_fails() {
        let key = EncryptionKey::from_bytes(&[42u8; 32]).unwrap();
        let mut encrypted = encrypt_bytes(&key, b"secret").unwrap();
        encrypted[20] ^= 0xFF; // Corrupt

        assert!(decrypt_bytes(&key, &encrypted).is_err());
    }

    #[test]
    fn test_different_nonces() {
        let key = EncryptionKey::from_bytes(&[42u8; 32]).unwrap();
        let plaintext = b"same message";

        let c1 = encrypt_bytes(&key, plaintext).unwrap();
        let c2 = encrypt_bytes(&key, plaintext).unwrap();
        assert_ne!(c1, c2); // Different nonces

        assert_eq!(decrypt_bytes(&key, &c1).unwrap(), plaintext);
        assert_eq!(decrypt_bytes(&key, &c2).unwrap(), plaintext);
    }
}
