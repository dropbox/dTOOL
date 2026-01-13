//! Utility functions for Redis vector store operations.

use dashflow::core::error::{Error, Result};

/// Encode a vector of f32 values to bytes for Redis storage.
///
/// Redis expects vectors as binary blobs. This function converts a slice of
/// f32 values to bytes in little-endian format.
#[must_use]
pub fn encode_vector(vector: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vector.len() * 4);
    for &value in vector {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

/// Decode bytes from Redis back to a vector of f32 values.
///
/// Reverses the encoding done by `encode_vector()`.
pub fn decode_vector(bytes: &[u8]) -> Result<Vec<f32>> {
    if bytes.len() % 4 != 0 {
        return Err(Error::config(format!(
            "Invalid vector bytes length: {} (must be multiple of 4)",
            bytes.len()
        )));
    }

    let mut vector = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        let value = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        vector.push(value);
    }
    Ok(vector)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_vector() {
        let original = vec![1.0, 2.5, -3.15, 0.0];
        let encoded = encode_vector(&original);
        let decoded = decode_vector(&encoded).unwrap();

        assert_eq!(original.len(), decoded.len());
        for (a, b) in original.iter().zip(decoded.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_decode_invalid_bytes() {
        let invalid = vec![1, 2, 3]; // Not a multiple of 4
        assert!(decode_vector(&invalid).is_err());
    }
}
